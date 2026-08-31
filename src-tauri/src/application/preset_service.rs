use crate::error::{require_db, require_found, validate_non_empty, ServiceError};
use crate::models::{AgentType, NewPreset, Preset, UpdatePreset};

/// Preset CRUD + switch（写 live 配置）。
/// 对齐 cc-switch ProviderService：SSOT（DB 为准），切换时由 harness writer
/// 注入网关信息后投影到 live 配置文件。
pub struct PresetService;

impl PresetService {
    pub async fn list(agent_type: String) -> Result<Vec<Preset>, ServiceError> {
        let pool = require_db()?;
        let rows = sqlx::query_as::<_, PresetRow>(
            "SELECT id, name, agent_type, settings_config, category, notes, sort_index, is_active, created_at, updated_at
             FROM presets WHERE agent_type = ? ORDER BY sort_index IS NULL, sort_index, created_at",
        )
        .bind(&agent_type)
        .fetch_all(pool)
        .await
        .map_err(ServiceError::from)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn get(profile_id: String) -> Result<Preset, ServiceError> {
        validate_non_empty("profile_id", &profile_id)?;
        let pool = require_db()?;
        let row = sqlx::query_as::<_, PresetRow>(
            "SELECT id, name, agent_type, settings_config, category, notes, sort_index, is_active, created_at, updated_at
             FROM presets WHERE id = ?",
        )
        .bind(&profile_id)
        .fetch_optional(pool)
        .await
        .map_err(ServiceError::from)?;
        require_found(row.map(Into::into), "Preset")
    }

    pub async fn create(payload: NewPreset) -> Result<Preset, ServiceError> {
        validate_non_empty("name", &payload.name)?;
        validate_non_empty("agent_type", &payload.agent_type)?;
        let pool = require_db()?;
        let id = uuid::Uuid::new_v4().to_string();
        let settings = serde_json::to_string(&payload.settings_config).map_err(|e| {
            ServiceError::BadRequest { message: format!("settings_config 序列化失败: {e}"), code: None }
        })?;
        sqlx::query(
            "INSERT INTO presets (id, name, agent_type, settings_config, category, notes, sort_index, is_active)
             VALUES (?, ?, ?, ?, ?, ?, ?, 0)",
        )
        .bind(&id)
        .bind(&payload.name)
        .bind(&payload.agent_type)
        .bind(&settings)
        .bind(&payload.category)
        .bind(&payload.notes)
        .bind(&payload.sort_index)
        .execute(pool)
        .await
        .map_err(ServiceError::from)?;
        Self::get(id).await
    }

    pub async fn update(preset_id: String, payload: UpdatePreset) -> Result<Preset, ServiceError> {
        validate_non_empty("preset_id", &preset_id)?;
        let pool = require_db()?;
        let existing = Self::get(preset_id.clone()).await?;

        let name = payload.name.unwrap_or(existing.name);
        let settings_config = payload
            .settings_config
            .map(|v| {
                serde_json::to_string(&v).map_err(|e| ServiceError::BadRequest {
                    message: format!("settings_config 序列化失败: {e}"),
                    code: None,
                })
            })
            .transpose()?
            .unwrap_or_else(|| serde_json::to_string(&existing.settings_config).unwrap_or_default());
        let category = payload.category.or(existing.category);
        let notes = payload.notes.or(existing.notes);
        let sort_index = payload.sort_index.or(existing.sort_index);

        sqlx::query(
            "UPDATE presets SET name = ?, settings_config = ?, category = ?, notes = ?, sort_index = ?, updated_at = datetime('now') WHERE id = ?",
        )
        .bind(&name)
        .bind(&settings_config)
        .bind(&category)
        .bind(&notes)
        .bind(&sort_index)
        .bind(&preset_id)
        .execute(pool)
        .await
        .map_err(ServiceError::from)?;
        Self::get(preset_id).await
    }

    pub async fn delete(preset_id: String) -> Result<bool, ServiceError> {
        validate_non_empty("preset_id", &preset_id)?;
        let pool = require_db()?;
        let result = sqlx::query("DELETE FROM presets WHERE id = ?")
            .bind(&preset_id)
            .execute(pool)
            .await
            .map_err(ServiceError::from)?;
        Ok(result.rows_affected() > 0)
    }

    /// 切换：写目标 preset 的 live 投影（注入网关 base_url/api_key）→ 更新 is_active。
    /// 写 live 失败时快照已由 writer 回滚，DB 状态不变。
    pub async fn switch(agent_type: String, preset_id: String) -> Result<SwitchResult, ServiceError> {
        let pool = require_db()?;
        let preset = Self::get(preset_id.clone()).await?;
        if preset.agent_type != agent_type {
            return Err(ServiceError::BadRequest {
                message: format!("Preset 的 agent_type ({}) 与请求 ({}) 不匹配", preset.agent_type, agent_type),
                code: None,
            });
        }

        let writer = crate::application::harness::writer_for(&agent_type).ok_or_else(|| {
            ServiceError::BadRequest {
                message: format!("该 Agent 类型 ({agent_type}) 不支持配置自动写入"),
                code: None,
            }
        })?;

        let home = crate::get_home_dir().to_path_buf();
        let live_path = writer.live_path(&home);
        tracing::debug!("[presets:switch] agent_type={agent_type} preset_id={preset_id} live={}", live_path.display());

        // backfill：切走前把 live 里的用户手工改动（剥离注入字段后）回填到当前激活 preset，
        // 避免切换后丢失用户直接在应用里改的内容（对齐 cc-switch sync_common_config_snippet_from_live）
        Self::backfill_current_live(pool, &agent_type, writer.as_ref(), &home).await?;

        // 注入网关 base_url/api_key（按 harness 映射到正确字段位置）
        let mut effective = preset.settings_config.clone();
        if let Ok(settings) = crate::models::GatewaySettings::load(
            crate::get_settings_path().ok_or_else(|| ServiceError::Internal {
                message: "无法获取设置路径".to_string(),
                detail: None,
            })?,
        ) {
            let base_url = format!("http://{}:{}/v1", settings.bind_host, settings.bind_port);
            let api_key = crate::application::gateway_key_service::builtin_key_value();
            Self::inject_gateway_config(&agent_type, &mut effective, &base_url, &api_key);
        }

        // 写 live（writer 内部快照回滚）
        writer
            .write_live(&home, &effective)
            .await
            .map_err(|e| ServiceError::Internal {
                message: format!("写入 live 配置失败: {e}"),
                detail: None,
            })?;
        tracing::info!("[presets:switch] 写入 live 配置成功: {}", live_path.display());

        // 更新 is_active（deactivate_all + activate）
        sqlx::query("UPDATE presets SET is_active = 0 WHERE agent_type = ?")
            .bind(&agent_type)
            .execute(pool)
            .await
            .map_err(ServiceError::from)?;
        sqlx::query("UPDATE presets SET is_active = 1, updated_at = datetime('now') WHERE id = ?")
            .bind(&preset_id)
            .execute(pool)
            .await
            .map_err(ServiceError::from)?;

        let requires_restart = AgentType::requires_restart(&agent_type);
        let mut warnings = Vec::new();
        if requires_restart {
            warnings.push("请重启终端/应用以使配置生效".to_string());
        }
        Ok(SwitchResult { success: true, warnings, requires_restart })
    }

    /// backfill：切走前把 live 配置中的用户手工改动回填到当前激活 preset。
    /// 剥离注入字段（env 网关键 / base_url / api_key / _silk_managed）后，
    /// 仅当 live 有内容且存在当前激活 preset 时更新其 settings_config。
    async fn backfill_current_live(
        pool: &sqlx::SqlitePool,
        agent_type: &str,
        writer: &dyn crate::application::harness::HarnessWriter,
        home: &std::path::Path,
    ) -> Result<(), ServiceError> {
        // 当前激活 preset
        let current: Option<Preset> = sqlx::query_as::<_, PresetRow>(
            "SELECT id, name, agent_type, settings_config, category, notes, sort_index, is_active, created_at, updated_at
             FROM presets WHERE agent_type = ? AND is_active = 1",
        )
        .bind(agent_type)
        .fetch_optional(pool)
        .await
        .map_err(ServiceError::from)?
        .map(Into::into);

        let Some(mut current) = current else {
            return Ok(());
        };

        // 读 live
        let Some(data) = writer.read_live(home).await.map_err(|e| ServiceError::Internal {
            message: format!("读取 live 配置失败: {e}"),
            detail: None,
        })?
        else {
            return Ok(());
        };
        let text = String::from_utf8(data).map_err(|_| ServiceError::Internal {
            message: "live 配置不是合法 UTF-8 文本".to_string(),
            detail: None,
        })?;

        // 按格式解析并剥离注入字段
        let value: serde_json::Value = match writer.config_format() {
            crate::application::config_writer::ConfigFormat::Json => {
                serde_json::from_str(&text).map_err(|e| ServiceError::BadRequest {
                    message: format!("live 配置解析失败: {e}"),
                    code: None,
                })?
            }
            crate::application::config_writer::ConfigFormat::Toml => {
                toml::from_str(&text).map_err(|e| ServiceError::BadRequest {
                    message: format!("live 配置解析失败: {e}"),
                    code: None,
                })?
            }
            crate::application::config_writer::ConfigFormat::Yaml => {
                serde_yaml::from_str(&text).map_err(|e| ServiceError::BadRequest {
                    message: format!("live 配置解析失败: {e}"),
                    code: None,
                })?
            }
        };
        let stripped = strip_injected_fields(agent_type, &value);
        if stripped.is_null() {
            return Ok(());
        }

        current.settings_config = stripped;
        let settings = serde_json::to_string(&current.settings_config).map_err(|e| {
            ServiceError::Internal { message: format!("settings_config 序列化失败: {e}"), detail: None }
        })?;
        sqlx::query("UPDATE presets SET settings_config = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(&settings)
            .bind(&current.id)
            .execute(pool)
            .await
            .map_err(ServiceError::from)?;
        tracing::debug!("[presets:backfill] agent_type={agent_type} 已回填 live 改动到 preset {}", current.id);
        Ok(())
    }

    /// 注入网关 base_url/api_key（按 harness 映射到正确字段位置）。
    /// 仅添加缺失项，不覆盖用户已存在的键。
    pub fn inject_gateway_config(
        agent_type: &str,
        effective: &mut serde_json::Value,
        base_url: &str,
        api_key: &str,
    ) {
        let Some(obj) = effective.as_object_mut() else { return };
        match agent_type {
            // Claude Code：写入 env 子对象（settings.json 的 env 作为环境变量注入）
            // Gemini CLI：写入 env 子对象
            "claude_code" | "gemini_cli" => {
                let env = obj
                    .entry("env".to_string())
                    .or_insert_with(|| serde_json::json!({}));
                if let Some(e) = env.as_object_mut() {
                    if matches!(agent_type, "claude_code") {
                        e.entry("ANTHROPIC_BASE_URL".to_string())
                            .or_insert_with(|| serde_json::json!(base_url));
                        e.entry("ANTHROPIC_AUTH_TOKEN".to_string())
                            .or_insert_with(|| serde_json::json!(api_key));
                    } else {
                        e.entry("GOOGLE_GEMINI_BASE_URL".to_string())
                            .or_insert_with(|| serde_json::json!(base_url));
                        e.entry("GEMINI_API_KEY".to_string())
                            .or_insert_with(|| serde_json::json!(api_key));
                    }
                }
            }
            // Codex：顶层注入（CodexWriter 投影到 model_providers 表）
            // OpenCode / Hermes：整个 settings 即 provider 条目内容，顶层注入即条目内
            _ => {
                obj.entry("base_url".to_string())
                    .or_insert_with(|| serde_json::json!(base_url));
                obj.entry("api_key".to_string())
                    .or_insert_with(|| serde_json::json!(api_key));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 行映射
// ---------------------------------------------------------------------------

/// 剥离 live 配置中 silk 注入的字段（backfill 用）：
/// - claude_code/gemini_cli：剥离 env 内的网关键 + _silk_managed
/// - codex/opencode/hermes：剥离顶层 base_url/api_key + _silk_managed
fn strip_injected_fields(agent_type: &str, value: &serde_json::Value) -> serde_json::Value {
    let mut v = value.clone();
    match agent_type {
        "claude_code" | "gemini_cli" => {
            if let Some(o) = v.as_object_mut() {
                if let Some(env) = o.get_mut("env").and_then(|e| e.as_object_mut()) {
                    env.remove("ANTHROPIC_BASE_URL");
                    env.remove("ANTHROPIC_AUTH_TOKEN");
                    env.remove("GOOGLE_GEMINI_BASE_URL");
                    env.remove("GEMINI_API_KEY");
                    if env.is_empty() {
                        o.remove("env");
                    }
                }
                o.remove("_silk_managed");
            }
        }
        _ => {
            if let Some(o) = v.as_object_mut() {
                o.remove("base_url");
                o.remove("api_key");
                o.remove("openai_base_url");
                o.remove("_silk_managed");
            }
        }
    }
    v
}

#[derive(sqlx::FromRow)]
struct PresetRow {
    id: String,
    name: String,
    agent_type: String,
    settings_config: String,
    category: Option<String>,
    notes: Option<String>,
    sort_index: Option<i64>,
    is_active: bool,
    created_at: Option<String>,
    updated_at: Option<String>,
}

impl From<PresetRow> for Preset {
    fn from(row: PresetRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            agent_type: row.agent_type,
            settings_config: serde_json::from_str(&row.settings_config)
                .unwrap_or(serde_json::Value::Null),
            category: row.category,
            notes: row.notes,
            sort_index: row.sort_index,
            is_active: row.is_active,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// 切换结果（对齐 cc-switch SwitchResult）
#[derive(Debug, Clone, serde::Serialize)]
pub struct SwitchResult {
    pub success: bool,
    pub warnings: Vec<String>,
    pub requires_restart: bool,
}
