use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::application::gateway_service;
use crate::error::{bad_request, require_db, ServiceError};
use crate::models::GatewaySettings;
use crate::persistence::config_transfer_repo::{ConfigExportBundle, ConfigTransferRepo, PortableProvider};
use crate::persistence::{ModelMappingRepo, ProviderRepo};
use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportConfigPayload {
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportConfigPayload {
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupDatabasePayload {
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreDatabasePayload {
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOperationResponse {
    pub file_path: String,
}

pub async fn export_config(payload: ExportConfigPayload) -> Result<FileOperationResponse, ServiceError> {
    let pool = require_db()?;
    let settings_path = crate::get_settings_path().ok_or_else(|| ServiceError::Internal {
        message: "网关设置路径未初始化".to_string(),
        detail: None,
    })?;

    let bundle = ConfigExportBundle {
        schema_version: 1,
        exported_at: chrono::Utc::now().to_rfc3339(),
        gateway_settings: crate::persistence::GatewaySettingsRepo::load_effective(settings_path),
        providers: ProviderRepo::find_all(pool)
            .await?
            .into_iter()
            .map(PortableProvider::from_provider)
            .collect::<Result<Vec<_>, _>>()?,
        model_mappings: ModelMappingRepo::find_all(pool).await?,
        model_mapping_channels: ConfigTransferRepo::find_all_model_mapping_channels(pool).await?,
    };

    let path = resolve_output_path(
        payload.file_path,
        "silk_config_export",
        "json",
        settings_path.parent(),
    )?;

    let content = serde_json::to_string_pretty(&bundle).map_err(|e| ServiceError::Internal {
        message: "导出配置序列化失败".to_string(),
        detail: Some(e.to_string()),
    })?;

    std::fs::write(&path, content).map_err(|e| ServiceError::Internal {
        message: "写入配置文件失败".to_string(),
        detail: Some(e.to_string()),
    })?;

    Ok(FileOperationResponse {
        file_path: path.display().to_string(),
    })
}

pub async fn import_config(
    state: &AppState,
    payload: ImportConfigPayload,
) -> Result<FileOperationResponse, ServiceError> {
    if payload.file_path.trim().is_empty() {
        return bad_request("导入路径不能为空");
    }

    let pool = require_db()?;
    let import_path = PathBuf::from(payload.file_path.trim());
    if !import_path.exists() {
        return bad_request("导入文件不存在");
    }

    let content = std::fs::read_to_string(&import_path).map_err(|e| ServiceError::Internal {
        message: "读取导入文件失败".to_string(),
        detail: Some(e.to_string()),
    })?;
    let bundle: ConfigExportBundle =
        serde_json::from_str(&content).map_err(|e| ServiceError::BadRequest {
            message: "导入文件格式无效".to_string(),
            code: Some(e.to_string()),
        })?;

    if bundle.schema_version != 1 {
        return bad_request("暂不支持该配置版本");
    }

    // 先写 JSON 设置文件，再写 DB，避免 DB 已提交但 JSON 写入失败的不一致
    let settings_path = crate::get_settings_path().ok_or_else(|| ServiceError::Internal {
        message: "网关设置路径未初始化".to_string(),
        detail: None,
    })?;
    bundle
        .gateway_settings
        .save(settings_path)
        .map_err(|e| ServiceError::Internal {
            message: "写入网关设置失败".to_string(),
            detail: Some(e),
        })?;

    // 全量替换：清空三张配置表后按导入包重建，整个过程在同一事务内完成
    with_gateway_stop_guard(state, || async {
        let mut tx = pool.begin().await?;
        ConfigTransferRepo::replace_all_with_bundle(&mut tx, &bundle).await?;
        tx.commit().await?;
        Ok(())
    })
    .await?;

    apply_gateway_settings(state, bundle.gateway_settings.clone()).await;

    Ok(FileOperationResponse {
        file_path: import_path.display().to_string(),
    })
}

pub async fn backup_database(
    payload: BackupDatabasePayload,
) -> Result<FileOperationResponse, ServiceError> {
    let pool = require_db()?;
    let db_path = crate::get_db_path().ok_or_else(|| ServiceError::Internal {
        message: "数据库路径未初始化".to_string(),
        detail: None,
    })?;

    let target = resolve_output_path(
        payload.file_path,
        "silk_database_backup",
        "db",
        db_path.parent(),
    )?;

    ConfigTransferRepo::wal_checkpoint(pool).await?;

    std::fs::copy(db_path, &target).map_err(|e| ServiceError::Internal {
        message: "备份数据库失败".to_string(),
        detail: Some(e.to_string()),
    })?;

    Ok(FileOperationResponse {
        file_path: target.display().to_string(),
    })
}

pub async fn restore_database(
    state: &AppState,
    payload: RestoreDatabasePayload,
) -> Result<FileOperationResponse, ServiceError> {
    if payload.file_path.trim().is_empty() {
        return bad_request("恢复路径不能为空");
    }

    let pool = require_db()?;
    let backup_path = PathBuf::from(payload.file_path.trim());
    if !backup_path.exists() {
        return bad_request("备份文件不存在");
    }

    let backup_path_clone = backup_path.clone();

    with_gateway_stop_guard(state, || async {
        let mut tx = pool.begin().await?;
        ConfigTransferRepo::restore_from_backup(&mut tx, &backup_path_clone).await?;
        tx.commit().await?;
        Ok(())
    })
    .await?;

    // 尝试从备份 DB 恢复网关设置（旧备份可能有 gateway_settings 表，新备份没有）
    let _ = restore_settings_from_backup_db(state, &backup_path).await;

    {
        let gateway = state.gateway.read().await;
        gateway.provider_cache.clear().await;
    }
    state.refresh_lookup().await;

    Ok(FileOperationResponse {
        file_path: backup_path.display().to_string(),
    })
}

/// 确保操作完成后（无论成功或失败）网关恢复到操作前的运行状态
async fn with_gateway_stop_guard<F, Fut, T>(
    state: &AppState,
    f: F,
) -> Result<T, ServiceError>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, ServiceError>>,
{
    let was_running = state.gateway_server.read().await.is_some();
    if was_running {
        let _ = gateway_service::stop(state).await;
    }

    let result = f().await;

    if was_running {
        let _ = gateway_service::start_existing_gateway(state).await;
    }

    result
}

/// 尝试从备份数据库文件中恢复 gateway_settings 到 JSON 文件并同步内存状态
async fn restore_settings_from_backup_db(
    state: &AppState,
    backup_path: &Path,
) -> Result<(), ServiceError> {
    let settings_path = crate::get_settings_path().ok_or_else(|| ServiceError::Internal {
        message: "网关设置路径未初始化".to_string(),
        detail: None,
    })?;

    // 旧备份才有 gateway_settings 表，新备份已迁移为 JSON 文件存储
    let Some(settings) = ConfigTransferRepo::load_settings_from_backup_db(backup_path).await? else {
        return Ok(());
    };

    settings
        .save(settings_path)
        .map_err(|e| ServiceError::Internal {
            message: "写入网关设置失败".to_string(),
            detail: Some(e),
        })?;

    apply_gateway_settings(state, settings).await;
    Ok(())
}

/// 将设置应用到运行时内存：刷新共享设置、限流配置与字典缓存
async fn apply_gateway_settings(state: &AppState, settings: GatewaySettings) {
    {
        let gateway = state.gateway.read().await;
        {
            let mut current_settings = gateway.settings.write().await;
            *current_settings = settings.clone();
        }
        gateway
            .rate_limit_state
            .update_config(
                settings.rate_limit_enabled,
                settings.rate_limit_max_requests_per_minute as u64,
                settings.rate_limit_max_tokens_per_minute as u64,
            )
            .await;
    }
    state.refresh_lookup().await;
}

fn resolve_output_path(
    custom: Option<String>,
    prefix: &str,
    ext: &str,
    fallback_dir: Option<&Path>,
) -> Result<PathBuf, ServiceError> {
    if let Some(path) = custom.filter(|p| !p.trim().is_empty()) {
        let path = PathBuf::from(path.trim());
        ensure_parent_dir(&path)?;
        return Ok(path);
    }

    let dir = fallback_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&dir).map_err(|e| ServiceError::Internal {
        message: "创建导出目录失败".to_string(),
        detail: Some(e.to_string()),
    })?;
    Ok(dir.join(format!(
        "{}_{}.{}",
        prefix,
        chrono::Local::now().format("%Y%m%d_%H%M%S"),
        ext
    )))
}

fn ensure_parent_dir(path: &Path) -> Result<(), ServiceError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ServiceError::Internal {
            message: "创建目录失败".to_string(),
            detail: Some(e.to_string()),
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::crypto::decrypt;
    use crate::models::{Provider, ProviderKeyEntry};
    use crate::persistence::config_transfer_repo::PortableProvider;

    #[test]
    fn portable_provider_reencrypts_plain_keys_on_import() {
        let portable = PortableProvider {
            id: "p1".into(),
            name: "test".into(),
            protocols: "[]".into(),
            models: "[]".into(),
            keys: serde_json::to_string(&vec![ProviderKeyEntry {
                name: "main".into(),
                value: "secret".into(),
                enabled: true,
                weight: 1,
            }])
            .unwrap(),
            key_strategy: "round_robin".into(),
            api_base_url: "https://example.com".into(),
            proxy_url: None,
            timeout_seconds: 30,
            max_retries: 2,
            status: "enabled".into(),
            health_status: None,
            last_health_check_at: None,
            metadata_json: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };

        let encrypted = portable.encrypted_keys_json().unwrap();
        let keys: Vec<ProviderKeyEntry> = serde_json::from_str(&encrypted).unwrap();
        assert_eq!(decrypt(&keys[0].value).unwrap(), "secret");
    }

    #[test]
    fn portable_export_decrypts_provider_keys() {
        let encrypted_value = crate::crypto::encrypt("provider-secret").unwrap();
        let provider = Provider {
            id: "p1".into(),
            name: "provider".into(),
            protocols: "[]".into(),
            models: "[]".into(),
            keys: serde_json::to_string(&vec![ProviderKeyEntry {
                name: "main".into(),
                value: encrypted_value,
                enabled: true,
                weight: 1,
            }])
            .unwrap(),
            key_strategy: "round_robin".into(),
            api_base_url: "https://example.com".into(),
            proxy_url: None,
            timeout_seconds: 30,
            max_retries: 2,
            status: "enabled".into(),
            health_status: None,
            last_health_check_at: None,
            metadata_json: None,
            custom_headers: "[]".to_string(),
            models_passthrough: 0,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };

        let portable = PortableProvider::from_provider(provider).unwrap();
        let keys: Vec<ProviderKeyEntry> = serde_json::from_str(&portable.keys).unwrap();
        assert_eq!(keys[0].value, "provider-secret");
    }
}
