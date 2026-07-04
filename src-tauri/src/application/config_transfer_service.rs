use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::application::gateway_service;
use crate::error::{bad_request, require_db, ServiceError};
use crate::models::{
    GatewayKey, GatewaySettings, ModelMapping, ModelMappingChannel, Provider, RoutingRule,
};
use crate::persistence::{GatewayKeyRepo, ModelMappingRepo, ProviderRepo, RoutingRuleRepo};
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

#[derive(Debug, Clone, Serialize)]
pub struct FileOperationResponse {
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConfigExportBundle {
    schema_version: i64,
    exported_at: String,
    gateway_settings: GatewaySettings,
    providers: Vec<Provider>,
    routing_rules: Vec<RoutingRule>,
    model_mappings: Vec<ModelMapping>,
    model_mapping_channels: Vec<ModelMappingChannel>,
    gateway_keys: Vec<GatewayKey>,
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
        providers: ProviderRepo::find_all(pool).await?,
        routing_rules: RoutingRuleRepo::find_all(pool).await?,
        model_mappings: ModelMappingRepo::find_all(pool).await?,
        model_mapping_channels: sqlx::query_as::<_, ModelMappingChannel>(
            r#"SELECT * FROM model_mapping_channels ORDER BY created_at ASC"#,
        )
        .fetch_all(pool)
        .await?,
        gateway_keys: GatewayKeyRepo::find_all(pool).await?,
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

    let was_running = state.gateway_server.read().await.is_some();
    if was_running {
        let _ = gateway_service::stop(state).await;
    }

    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM model_mapping_channels").execute(&mut *tx).await?;
    sqlx::query("DELETE FROM model_mappings").execute(&mut *tx).await?;
    sqlx::query("DELETE FROM routing_rules").execute(&mut *tx).await?;
    sqlx::query("DELETE FROM providers").execute(&mut *tx).await?;
    sqlx::query("DELETE FROM gateway_keys").execute(&mut *tx).await?;

    for provider in &bundle.providers {
        sqlx::query(
            r#"
            INSERT INTO providers (
                id, name, protocols, models, keys, key_strategy, api_base_url,
                proxy_url, timeout_seconds, max_retries, status, health_status,
                last_health_check_at, metadata_json, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
            "#,
        )
        .bind(&provider.id)
        .bind(&provider.name)
        .bind(&provider.protocols)
        .bind(&provider.models)
        .bind(&provider.keys)
        .bind(&provider.key_strategy)
        .bind(&provider.api_base_url)
        .bind(&provider.proxy_url)
        .bind(provider.timeout_seconds)
        .bind(provider.max_retries)
        .bind(&provider.status)
        .bind(&provider.health_status)
        .bind(provider.last_health_check_at)
        .bind(&provider.metadata_json)
        .bind(provider.created_at)
        .bind(provider.updated_at)
        .execute(&mut *tx)
        .await?;
    }

    for rule in &bundle.routing_rules {
        sqlx::query(
            r#"
            INSERT INTO routing_rules (
                id, name, match_host, match_path, match_method, match_content_type,
                inbound_protocol, outbound_protocol, target_provider_id, target_group_id,
                failover_provider_id, protocol_conversion, model_name_override, metadata_json,
                priority, enabled, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
            "#,
        )
        .bind(&rule.id)
        .bind(&rule.name)
        .bind(&rule.match_host)
        .bind(&rule.match_path)
        .bind(&rule.match_method)
        .bind(&rule.match_content_type)
        .bind(&rule.inbound_protocol)
        .bind(&rule.outbound_protocol)
        .bind(&rule.target_provider_id)
        .bind(&rule.target_group_id)
        .bind(&rule.failover_provider_id)
        .bind(rule.protocol_conversion)
        .bind(&rule.model_name_override)
        .bind(&rule.metadata_json)
        .bind(rule.priority)
        .bind(rule.enabled)
        .bind(rule.created_at)
        .bind(rule.updated_at)
        .execute(&mut *tx)
        .await?;
    }

    for mapping in &bundle.model_mappings {
        sqlx::query(
            r#"
            INSERT INTO model_mappings (
                id, model_name, max_input_tokens, max_context_tokens, max_output_tokens,
                input_price_per_1m, output_price_per_1m, capabilities, description,
                vendor, knowledge_cutoff, model_family, reference_url,
                strategy, enabled, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
            "#,
        )
        .bind(&mapping.id)
        .bind(&mapping.model_name)
        .bind(mapping.max_input_tokens)
        .bind(mapping.max_context_tokens)
        .bind(mapping.max_output_tokens)
        .bind(mapping.input_price_per_1m)
        .bind(mapping.output_price_per_1m)
        .bind(&mapping.capabilities)
        .bind(&mapping.description)
        .bind(&mapping.vendor)
        .bind(&mapping.knowledge_cutoff)
        .bind(&mapping.model_family)
        .bind(&mapping.reference_url)
        .bind(&mapping.strategy)
        .bind(mapping.enabled)
        .bind(mapping.created_at)
        .bind(mapping.updated_at)
        .execute(&mut *tx)
        .await?;
    }

    for channel in &bundle.model_mapping_channels {
        sqlx::query(
            r#"
            INSERT INTO model_mapping_channels (
                id, mapping_id, provider_id, selected_models, enabled, created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(&channel.id)
        .bind(&channel.mapping_id)
        .bind(&channel.provider_id)
        .bind(&channel.selected_models)
        .bind(channel.enabled)
        .bind(channel.created_at)
        .execute(&mut *tx)
        .await?;
    }

    for key in &bundle.gateway_keys {
        sqlx::query(
            r#"
            INSERT INTO gateway_keys (
                id, name, key_hash, key_prefix, enabled, expires_at, max_concurrent, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
        )
        .bind(&key.id)
        .bind(&key.name)
        .bind(&key.key_hash)
        .bind(&key.key_prefix)
        .bind(key.enabled)
        .bind(key.expires_at)
        .bind(key.max_concurrent)
        .bind(key.created_at)
        .bind(key.updated_at)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

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

    {
        let gateway = state.gateway.read().await;
        {
            let mut current_settings = gateway.settings.write().await;
            *current_settings = bundle.gateway_settings.clone();
        }
        gateway.provider_cache.clear().await;
        gateway.route_manager.reload(pool).await?;
        gateway
            .rate_limit_state
            .update_config(
                bundle.gateway_settings.rate_limit_enabled,
                bundle.gateway_settings.rate_limit_max_requests_per_minute as u64,
                bundle.gateway_settings.rate_limit_max_tokens_per_minute as u64,
            )
            .await;
    }
    state.refresh_lookup().await;

    if was_running {
        let _ = gateway_service::start_existing_gateway(state).await;
    }

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

    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(pool)
        .await?;

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

    let was_running = state.gateway_server.read().await.is_some();
    if was_running {
        let _ = gateway_service::stop(state).await;
    }

    let mut tx = pool.begin().await?;

    sqlx::query("PRAGMA foreign_keys = OFF").execute(&mut *tx).await?;
    sqlx::query("ATTACH DATABASE ?1 AS restore_db")
        .bind(backup_path.to_string_lossy().to_string())
        .execute(&mut *tx)
        .await?;

    const TABLES: &[&str] = &[
        "request_log_extra_token",
        "request_logs",
        "model_mapping_channels",
        "model_mappings",
        "routing_rules",
        "gateway_keys",
        "providers",
        "gateway_settings",
    ];

    for table in TABLES {
        let delete_sql = format!("DELETE FROM {table}");
        sqlx::query(&delete_sql).execute(&mut *tx).await?;
    }

    for table in TABLES.iter().rev() {
        let copy_sql = format!("INSERT INTO main.{0} SELECT * FROM restore_db.{0}", table);
        sqlx::query(&copy_sql).execute(&mut *tx).await?;
    }

    sqlx::query("DETACH DATABASE restore_db")
        .execute(&mut *tx)
        .await?;
    sqlx::query("PRAGMA foreign_keys = ON").execute(&mut *tx).await?;
    tx.commit().await?;

    {
        let gateway = state.gateway.read().await;
        gateway.provider_cache.clear().await;
        gateway.route_manager.reload(pool).await?;
    }
    state.refresh_lookup().await;

    if was_running {
        let _ = gateway_service::start_existing_gateway(state).await;
    }

    Ok(FileOperationResponse {
        file_path: backup_path.display().to_string(),
    })
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
