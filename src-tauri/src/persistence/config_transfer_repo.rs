use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool, Transaction, Sqlite};

use crate::crypto;
use crate::error::ServiceError;
use crate::models::{
    GatewaySettings, ModelMapping, ModelMappingChannel, Provider, ProviderKeyEntry,
};

/// 配置导入/导出包（JSON 序列化结构）。
///
/// 与 `PortableProvider` 一并放在持久化层，因为它们是配置迁移（备份/恢复/导入/导出）
/// 这一数据访问场景的专属 DTO，且其字段直接对应 SQL 读写。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigExportBundle {
    pub schema_version: i64,
    pub exported_at: String,
    pub gateway_settings: GatewaySettings,
    pub providers: Vec<PortableProvider>,
    pub model_mappings: Vec<ModelMapping>,
    pub model_mapping_channels: Vec<ModelMappingChannel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortableProvider {
    pub id: String,
    pub name: String,
    pub protocols: String,
    pub models: String,
    pub keys: String,
    pub key_strategy: String,
    pub api_base_url: String,
    pub proxy_url: Option<String>,
    pub timeout_seconds: i64,
    pub max_retries: i64,
    pub status: String,
    pub health_status: Option<String>,
    pub last_health_check_at: Option<chrono::NaiveDateTime>,
    pub metadata_json: Option<String>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

/// 配置迁移（导出/导入/备份/恢复）相关的全部数据访问。
///
/// 将原本散落在 `application::config_transfer_service` 中的裸 `sqlx` 查询集中到本仓库，
/// 满足「SQL 只出现在 persistence 层」的架构约束。
pub struct ConfigTransferRepo;

impl ConfigTransferRepo {
    /// 读取所有 model_mapping_channels（用于导出）
    pub async fn find_all_model_mapping_channels(
        pool: &SqlitePool,
    ) -> Result<Vec<ModelMappingChannel>, sqlx::Error> {
        sqlx::query_as::<_, ModelMappingChannel>(
            r#"SELECT * FROM model_mapping_channels ORDER BY created_at ASC"#,
        )
        .fetch_all(pool)
        .await
    }

    /// 在同一个事务内全量替换导入包中的三类配置
    ///
    /// 先清空 model_mapping_channels / model_mappings / providers，再按导入包重建，
    /// 保证导入要么完整生效，要么整体回滚。
    pub async fn replace_all_with_bundle(
        tx: &mut Transaction<'_, Sqlite>,
        bundle: &ConfigExportBundle,
    ) -> Result<(), ServiceError> {
        sqlx::query("DELETE FROM model_mapping_channels")
            .execute(&mut **tx)
            .await?;
        sqlx::query("DELETE FROM model_mappings")
            .execute(&mut **tx)
            .await?;
        sqlx::query("DELETE FROM providers")
            .execute(&mut **tx)
            .await?;

        insert_providers(&mut *tx, &bundle.providers).await?;
        insert_model_mappings(&mut *tx, &bundle.model_mappings).await?;
        insert_model_mapping_channels(&mut *tx, &bundle.model_mapping_channels).await?;

        Ok(())
    }

    /// 将 WAL 日志落盘，确保文件拷贝得到一致快照
    pub async fn wal_checkpoint(pool: &SqlitePool) -> Result<(), sqlx::Error> {
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(pool)
            .await?;
        Ok(())
    }

    /// 从备份数据库（ATTACH）把各配置表整体复制回主库
    ///
    /// 在调用方提供的事务内执行；`gateway_settings` 已迁移为 JSON 文件，不在复制范围内。
    pub async fn restore_from_backup(
        tx: &mut Transaction<'_, Sqlite>,
        backup_path: &Path,
    ) -> Result<(), ServiceError> {
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut **tx)
            .await?;
        sqlx::query("ATTACH DATABASE ?1 AS restore_db")
            .bind(backup_path.to_string_lossy().to_string())
            .execute(&mut **tx)
            .await?;

        // gateway_settings 已迁移为 JSON 文件存储，不再存在于 DB 中
        const TABLES: &[&str] = &[
            "request_log_extra_token",
            "request_logs",
            "model_mapping_channels",
            "model_mappings",
            "providers",
        ];

        for table in TABLES {
            let delete_sql = format!("DELETE FROM {table}");
            sqlx::query(&delete_sql).execute(&mut **tx).await?;
        }

        for table in TABLES.iter().rev() {
            let copy_sql = format!("INSERT INTO main.{0} SELECT * FROM restore_db.{0}", table);
            sqlx::query(&copy_sql).execute(&mut **tx).await?;
        }

        sqlx::query("DETACH DATABASE restore_db")
            .execute(&mut **tx)
            .await?;
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&mut **tx)
            .await?;

        Ok(())
    }

    /// 从备份数据库文件中读取 gateway_settings（旧备份才有该表）
    pub async fn load_settings_from_backup_db(
        backup_path: &Path,
    ) -> Result<Option<GatewaySettings>, ServiceError> {
        let backup_url = format!("sqlite://{}", backup_path.display());
        let backup_pool = SqlitePool::connect(&backup_url).await.map_err(|e| {
            ServiceError::Internal {
                message: "无法连接备份数据库".to_string(),
                detail: Some(e.to_string()),
            }
        })?;

        let restored = read_gateway_settings_row(&backup_pool).await;
        backup_pool.close().await;
        restored
    }
}

/// 批量写入 providers（API Key 在写入前重新加密）
async fn insert_providers(
    tx: &mut Transaction<'_, Sqlite>,
    providers: &[PortableProvider],
) -> Result<(), ServiceError> {
    for provider in providers {
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
        .bind(provider.encrypted_keys_json()?)
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
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

/// 批量写入 model_mappings
async fn insert_model_mappings(
    tx: &mut Transaction<'_, Sqlite>,
    mappings: &[ModelMapping],
) -> Result<(), ServiceError> {
    for mapping in mappings {
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
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

/// 批量写入 model_mapping_channels
async fn insert_model_mapping_channels(
    tx: &mut Transaction<'_, Sqlite>,
    channels: &[ModelMappingChannel],
) -> Result<(), ServiceError> {
    for channel in channels {
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
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

/// 读取 gateway_settings 表首行并映射为 GatewaySettings
///
/// 表不存在（新备份）或没有数据行时返回 `Ok(None)`。
async fn read_gateway_settings_row(
    backup_pool: &SqlitePool,
) -> Result<Option<GatewaySettings>, ServiceError> {
    let table_exists: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='gateway_settings'",
    )
    .fetch_one(backup_pool)
    .await
    .unwrap_or(false);

    if !table_exists {
        return Ok(None);
    }

    let row = sqlx::query(
        r#"SELECT bind_host, bind_port, allow_remote, log_retention_days,
           launch_at_startup, minimize_to_tray, close_to_tray, auto_start_gateway,
           default_provider_id,
           rate_limit_enabled, rate_limit_max_requests_per_minute, rate_limit_max_tokens_per_minute
           FROM gateway_settings LIMIT 1"#,
    )
    .fetch_optional(backup_pool)
    .await
    .map_err(|e| ServiceError::Internal {
        message: "读取备份中的网关设置失败".to_string(),
        detail: Some(e.to_string()),
    })?;

    let Some(row) = row else {
        return Ok(None);
    };

    Ok(Some(GatewaySettings {
        bind_host: row.get::<String, _>("bind_host"),
        bind_port: row.get::<i64, _>("bind_port"),
        allow_remote: row.get::<bool, _>("allow_remote"),
        log_retention_days: row.get::<i64, _>("log_retention_days"),
        launch_at_startup: row.get::<bool, _>("launch_at_startup"),
        minimize_to_tray: row.get::<bool, _>("minimize_to_tray"),
        close_to_tray: row.get::<bool, _>("close_to_tray"),
        auto_start_gateway: row.get::<bool, _>("auto_start_gateway"),
        default_provider_id: row.get::<Option<String>, _>("default_provider_id"),
        rate_limit_enabled: row.get::<bool, _>("rate_limit_enabled"),
        rate_limit_max_requests_per_minute: row
            .get::<i64, _>("rate_limit_max_requests_per_minute"),
        rate_limit_max_tokens_per_minute: row
            .get::<i64, _>("rate_limit_max_tokens_per_minute"),
        // 以下字段为后加列，旧备份里没有，缺失时取默认值
        trace_enabled: row.get::<Option<bool>, _>("trace_enabled").unwrap_or(false),
        log_level: row
            .get::<Option<String>, _>("log_level")
            .unwrap_or_else(|| "info".to_string()),
        file_level: row
            .get::<Option<String>, _>("file_level")
            .unwrap_or_else(|| "debug".to_string()),
        log_modules: HashMap::new(),
        // 后加字段，旧备份里没有，缺失时取默认值
        proxy_url: None,
    }))
}

impl PortableProvider {
    pub fn from_provider(provider: Provider) -> Result<Self, ServiceError> {
        let keys = provider
            .keys_vec()
            .into_iter()
            .map(|mut entry| {
                if !entry.value.is_empty() {
                    entry.value = crypto::decrypt(&entry.value).map_err(|e| {
                        ServiceError::Internal {
                            message: format!("导出渠道 Key 解密失败: {e}"),
                            detail: None,
                        }
                    })?;
                }
                Ok(entry)
            })
            .collect::<Result<Vec<_>, ServiceError>>()?;

        Ok(Self {
            id: provider.id,
            name: provider.name,
            protocols: provider.protocols,
            models: provider.models,
            keys: serde_json::to_string(&keys).map_err(|e| ServiceError::Internal {
                message: "导出渠道 Key 序列化失败".to_string(),
                detail: Some(e.to_string()),
            })?,
            key_strategy: provider.key_strategy,
            api_base_url: provider.api_base_url,
            proxy_url: provider.proxy_url,
            timeout_seconds: provider.timeout_seconds,
            max_retries: provider.max_retries,
            status: provider.status,
            health_status: provider.health_status,
            last_health_check_at: provider.last_health_check_at,
            metadata_json: provider.metadata_json,
            created_at: provider.created_at,
            updated_at: provider.updated_at,
        })
    }

    pub fn encrypted_keys_json(&self) -> Result<String, ServiceError> {
        let mut keys: Vec<ProviderKeyEntry> =
            serde_json::from_str(&self.keys).map_err(|e| ServiceError::BadRequest {
                message: "导入文件中的渠道 Key 格式无效".to_string(),
                code: Some(e.to_string()),
            })?;

        for entry in &mut keys {
            if !entry.value.is_empty() {
                entry.value = crypto::encrypt(&entry.value).map_err(|e| ServiceError::Internal {
                    message: format!("导入渠道 Key 加密失败: {e}"),
                    detail: None,
                })?;
            }
        }

        serde_json::to_string(&keys).map_err(|e| ServiceError::Internal {
            message: "导入渠道 Key 序列化失败".to_string(),
            detail: Some(e.to_string()),
        })
    }
}
