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

    /// 启动时把已有 harness live 配置导入 presets，避免首次打开预设页为空。
    /// 已存在的 preset 不覆盖；同一 harness/provider 只导入一次。
    pub async fn import_existing_live_configs(pool: &sqlx::SqlitePool, home: &std::path::Path) -> Result<(), ServiceError> {
        for &(agent_type, agent_name) in AgentType::all() {
            let Some(writer) = crate::application::harness::writer_for(agent_type) else { continue };
            let Some(data) = writer.read_live(home).await.map_err(|error| ServiceError::Internal { message: format!("读取 {agent_name} live 配置失败: {error}"), detail: None })? else { continue };
            let text = String::from_utf8(data).map_err(|error| ServiceError::Internal { message: format!("读取 {agent_name} live 配置失败: {error}"), detail: Some(error.to_string()) })?;
            let live = match Self::parse_live_config(&text, writer.config_format()) {
                Ok(value) => value,
                Err(error) => {
                    tracing::warn!(agent_type, error = %error, "跳过无法解析的现有 harness 配置");
                    continue;
                }
            };
            let mut imported = writer.extract_startup_settings(&live);
            if imported.is_empty() { continue; }
            imported.sort_by_key(|settings| !Self::is_managed_startup_entry(agent_type, &live, settings));

            let existing = sqlx::query_as::<_, PresetRow>(
                "SELECT id, name, agent_type, settings_config, category, notes, sort_index, is_active, created_at, updated_at
                 FROM presets WHERE agent_type = ?",
            )
            .bind(agent_type)
            .fetch_all(pool)
            .await
            .map_err(ServiceError::from)?;
            let mut active_claimed = existing.iter().any(|preset| preset.is_active);

            for settings in imported {
                let Some(identity) = Self::startup_identity(agent_type, &settings) else { continue };
                if existing.iter().any(|preset| Self::startup_identity(agent_type, &serde_json::from_str::<serde_json::Value>(&preset.settings_config).unwrap_or(serde_json::Value::Null)) == Some(identity.clone())) { continue; }
                let id = uuid::Uuid::new_v4().to_string();
                let is_active = if agent_type == "opencode" { false } else { !active_claimed };
                let name = Self::startup_preset_name(agent_type, agent_name, &identity);
                let settings_text = serde_json::to_string(&settings).map_err(|error| ServiceError::Internal { message: format!("保存 {agent_name} 导入配置失败: {error}"), detail: Some(error.to_string()) })?;
                sqlx::query(
                    "INSERT INTO presets (id, name, agent_type, settings_config, category, notes, sort_index, is_active)
                     VALUES (?, ?, ?, ?, 'imported', '启动时从现有 live 配置导入', NULL, ?)",
                )
                .bind(&id)
                .bind(&name)
                .bind(agent_type)
                .bind(&settings_text)
                .bind(is_active)
                .execute(pool)
                .await
                .map_err(ServiceError::from)?;
                active_claimed |= is_active;
                tracing::info!(agent_type, preset_id = %id, preset_name = %name, "启动时导入现有 harness 配置");
            }
        }
        Ok(())
    }
    fn parse_live_config(text: &str, format: crate::application::config_writer::ConfigFormat) -> Result<serde_json::Value, ServiceError> {
        let parsed = match format {
            crate::application::config_writer::ConfigFormat::Json => serde_json::from_str::<serde_json::Value>(text).map_err(|error| error.to_string()),
            crate::application::config_writer::ConfigFormat::Toml => toml::from_str::<toml::Value>(text).map_err(|error| error.to_string()).and_then(|value| serde_json::to_value(value).map_err(|error| error.to_string())),
            crate::application::config_writer::ConfigFormat::Yaml => serde_yaml::from_str::<serde_json::Value>(text).map_err(|error| error.to_string()),
        };
        parsed.map_err(|error| ServiceError::BadRequest { message: format!("live 配置解析失败: {error}"), code: None })
    }

    fn startup_identity(agent_type: &str, settings: &serde_json::Value) -> Option<String> {
        match agent_type {
            "opencode" => settings.get("id").and_then(|value| value.as_str()).filter(|value| !value.trim().is_empty()).map(ToOwned::to_owned),
            "hermes" => settings.get("name").and_then(|value| value.as_str()).filter(|value| !value.trim().is_empty()).map(ToOwned::to_owned),
            "codex" => settings.get("model_provider").and_then(|value| value.as_str()).filter(|value| !value.trim().is_empty()).map(ToOwned::to_owned),
            "claude_code" | "gemini_cli" => Some("default".to_string()),
            _ => None,
        }
    }

    fn startup_preset_name(agent_type: &str, agent_name: &str, identity: &str) -> String {
        match agent_type {
            "opencode" | "hermes" | "codex" => format!("{agent_name} · {identity}"),
            _ => format!("{agent_name} · 当前配置"),
        }
        .trim()
        .to_string()
    }

    fn is_managed_startup_entry(agent_type: &str, live: &serde_json::Value, settings: &serde_json::Value) -> bool {
        match agent_type {
            "opencode" => settings.get("id").and_then(|value| value.as_str()).and_then(|id| live.get("provider")?.get(id)?.get("_silk_managed")).and_then(|value| value.as_bool()).unwrap_or(false),
            "hermes" => settings.get("name").and_then(|value| value.as_str()).and_then(|name| live.get("custom_providers")?.as_array()?.iter().find(|entry| entry.get("name").and_then(|value| value.as_str()) == Some(name))?.get("_silk_managed")).and_then(|value| value.as_bool()).unwrap_or(false),
            _ => live.get("_silk_managed").and_then(|value| value.as_bool()).unwrap_or(false),
        }
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
        .bind(payload.sort_index)
        .execute(pool)
        .await
        .map_err(ServiceError::from)?;
        Self::get(id).await
    }

    /// 编辑激活预设 → 新 settings 注入网关后立即投影 live（DB→live on edit，对齐 cc-switch 双向同步），
    /// 免去「编辑后还需手动重新激活」。先写 live（失败 writer 已按快照回滚、DB 不动）再落 DB，
    /// 避免 DB/live 分裂状态。
    pub async fn update(preset_id: String, payload: UpdatePreset) -> Result<Preset, ServiceError> {
        validate_non_empty("preset_id", &preset_id)?;
        let pool = require_db()?;
        let existing = Self::get(preset_id.clone()).await?;

        let name = payload.name.clone().unwrap_or_else(|| existing.name.clone());
        let settings_config = payload
            .settings_config
            .clone()
            .map(|v| {
                serde_json::to_string(&v).map_err(|e| ServiceError::BadRequest {
                    message: format!("settings_config 序列化失败: {e}"),
                    code: None,
                })
            })
            .transpose()?
            .unwrap_or_else(|| serde_json::to_string(&existing.settings_config).unwrap_or_default());
        let category = payload.category.clone().or_else(|| existing.category.clone());
        let notes = payload.notes.clone().or_else(|| existing.notes.clone());
        let sort_index = payload.sort_index.or(existing.sort_index);

        if existing.is_active
            && crate::application::harness::writer_for(&existing.agent_type).is_some()
        {
            if let Some(new_settings) = payload.settings_config.as_ref() {
                // 剥离键：本预设旧声明、新 settings 不再声明的键（如移除某角色的模型）
                let old_keys = Self::collect_env_keys(&existing.agent_type, &existing.settings_config);
                let new_keys = Self::collect_env_keys(&existing.agent_type, new_settings);
                let remove_keys: Vec<String> = old_keys
                    .into_iter()
                    .filter(|key| !new_keys.contains(key))
                    .collect();
                if !remove_keys.is_empty() {
                    tracing::debug!(
                        "[presets:update] agent_type={} 剥离残留键 {:?}",
                        existing.agent_type,
                        remove_keys
                    );
                }
                Self::write_live_projection(&existing.agent_type, new_settings, &remove_keys).await?;
                tracing::info!("[presets:update] 激活预设 {preset_id} 已同步 live（编辑即生效）");
            }
        }

        sqlx::query(
            "UPDATE presets SET name = ?, settings_config = ?, category = ?, notes = ?, sort_index = ?, updated_at = datetime('now') WHERE id = ?",
        )
        .bind(&name)
        .bind(&settings_config)
        .bind(&category)
        .bind(&notes)
        .bind(sort_index)
        .bind(&preset_id)
        .execute(pool)
        .await
        .map_err(ServiceError::from)?;
        Self::get(preset_id).await
    }

    pub async fn delete(preset_id: String) -> Result<bool, ServiceError> {
        validate_non_empty("preset_id", &preset_id)?;
        let pool = require_db()?;
        let existing = Self::get(preset_id.clone()).await?;
        // 最小侵入（对齐 cc-switch）：当前激活的供应商不可删除，需先切换到其他预设
        if existing.is_active {
            return Err(ServiceError::BadRequest {
                message: "当前激活的预设不可删除，请先切换到其他预设".to_string(),
                code: Some("preset_active".to_string()),
            });
        }
        let result = sqlx::query("DELETE FROM presets WHERE id = ?")
            .bind(&preset_id)
            .execute(pool)
            .await
            .map_err(ServiceError::from)?;
        Ok(result.rows_affected() > 0)
    }

    /// 重排序（对齐 cc-switch update_sort_order）：
    /// 按传入 id 顺序写入 sort_index = 0..n-1，仅限该 agent_type 下的预设。
    pub async fn reorder(agent_type: String, ordered_ids: Vec<String>) -> Result<(), ServiceError> {
        validate_non_empty("agent_type", &agent_type)?;
        if ordered_ids.is_empty() {
            return Ok(());
        }
        let pool = require_db()?;
        // 预校验：所有 id 必须存在且属于该 agent_type（防跨 agent 篡改）
        let count: i64 = {
            let ids: Vec<String> = ordered_ids.to_vec();
            let placeholders = std::iter::repeat_n("?", ids.len()).collect::<Vec<_>>().join(",");
            let sql = format!("SELECT COUNT(*) FROM presets WHERE agent_type = ? AND id IN ({placeholders})");
            let mut q = sqlx::query_as::<_, (i64,)>(&sql).bind(&agent_type);
            for id in &ids {
                q = q.bind(id);
            }
            q.fetch_one(pool).await.map_err(ServiceError::from)?.0
        };
        if count as usize != ordered_ids.len() {
            return Err(ServiceError::BadRequest {
                message: "重排序列表包含不存在或不属于该 Agent 的预设".to_string(),
                code: Some("preset_reorder_mismatch".to_string()),
            });
        }
        for (i, id) in ordered_ids.iter().enumerate() {
            sqlx::query("UPDATE presets SET sort_index = ?, updated_at = datetime('now') WHERE id = ?")
                .bind(i as i64)
                .bind(id)
                .execute(pool)
                .await
                .map_err(ServiceError::from)?;
        }
        tracing::debug!("[presets:reorder] agent_type={agent_type} 已更新排序，共 {} 个", ordered_ids.len());
        Ok(())
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
        // 注入网关 base_url/api_key（按 harness 映射到正确字段位置）
        // OpenCode: 使用原始 id 字段作为 provider key，不覆盖为 UUID
        // _silk_provider_id 仅用于内部追踪，不写入 live 配置
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
        // 返回旧激活预设声明的 env 键（切换前捕获，backfill 会覆盖该行内容），用于计算残留键。
        let old_declared_keys =
            Self::backfill_current_live(pool, &agent_type, writer.as_ref(), &home).await?;

        // remove_keys = 旧预设声明、新预设不再声明的键：防止切到 B 后残留 A 的模型/端点键
        // （对齐 cc-switch 切换时 strip 旧 provider 独有字段）
        let new_keys = Self::collect_env_keys(&agent_type, &effective);
        let remove_keys: Vec<String> = old_declared_keys
            .into_iter()
            .filter(|k| !new_keys.contains(k))
            .collect();
        if !remove_keys.is_empty() {
            tracing::debug!(
                "[presets:switch] agent_type={agent_type} 剥离残留键 {} 个: {:?}",
                remove_keys.len(),
                remove_keys
            );
        }

        // 写 live（writer 内部快照回滚）
        writer
            .write_live(&home, &effective, &remove_keys)
            .await
            .map_err(|e| ServiceError::Internal {
                message: format!("写入 live 配置失败: {e}"),
                detail: None,
            })?;
        tracing::info!("[presets:switch] 写入 live 配置成功: {}", live_path.display());

        // OpenCode 是累加模式：写 live 配置但不改变 is_active，允许多个 preset 同时激活。
        if agent_type != "opencode" {
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
        }


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
    /// 返回切换前旧激活 preset 声明的 env 键（用于计算切换时的残留剥离）。
    async fn backfill_current_live(
        pool: &sqlx::SqlitePool,
        agent_type: &str,
        writer: &dyn crate::application::harness::HarnessWriter,
        home: &std::path::Path,
    ) -> Result<Vec<String>, ServiceError> {
        let current: Option<Preset> = sqlx::query_as::<_, PresetRow>(
            "SELECT id, name, agent_type, settings_config, category, notes, sort_index, is_active, created_at, updated_at
             FROM presets WHERE agent_type = ? AND is_active = 1",
        )
        .bind(agent_type)
        .fetch_optional(pool)
        .await
        .map_err(ServiceError::from)
        .map(|row| row.map(Into::into))?;
        let Some(mut current) = current else {
            return Ok(Vec::new());
        };
        let old_env_keys = Self::collect_env_keys(agent_type, &current.settings_config);
        let Some(data) = writer.read_live(home).await.map_err(|error| ServiceError::Internal {
            message: format!("读取 live 配置失败: {error}"),
            detail: None,
        })? else {
            return Ok(old_env_keys);
        };
        let text = String::from_utf8(data).map_err(|_| ServiceError::Internal {
            message: "live 配置不是合法 UTF-8 文本".to_string(),
            detail: None,
        })?;
        let value: serde_json::Value = match writer.config_format() {
            crate::application::config_writer::ConfigFormat::Json => serde_json::from_str(&text)
                .map_err(|error| error.to_string()),
            crate::application::config_writer::ConfigFormat::Toml => toml::from_str(&text)
                .map_err(|error| error.to_string()),
            crate::application::config_writer::ConfigFormat::Yaml => serde_yaml::from_str(&text)
                .map_err(|error| error.to_string()),
        }
        .map_err(|error| ServiceError::BadRequest {
            message: format!("live 配置解析失败: {error}"),
            code: None,
        })?;
        let Some(stripped) = writer.extract_settings(&value, &current.settings_config) else {
            return Ok(old_env_keys);
        };
        current.settings_config = stripped;
        let settings = serde_json::to_string(&current.settings_config).map_err(|error| ServiceError::Internal {
            message: format!("settings_config 序列化失败: {error}"),
            detail: None,
        })?;
        sqlx::query("UPDATE presets SET settings_config = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(&settings)
            .bind(&current.id)
            .execute(pool)
            .await
            .map_err(ServiceError::from)?;
        tracing::debug!("[presets:backfill] agent_type={agent_type} 已回填 live 改动到 preset {}", current.id);
        Ok(old_env_keys)
    }


    /// 提取 settings_config 中声明、会投影到 live env 的键（claude_code/gemini_cli）：
    /// env 子对象键 + 顶层兜底键（writer 会把两处都合并进 live env）。
    /// 其余 agent（codex/opencode/hermes 整条替换）无需键级剥离，返回空。
    fn collect_env_keys(agent_type: &str, settings: &serde_json::Value) -> Vec<String> {
        if !matches!(agent_type, "claude_code" | "gemini_cli") {
            return Vec::new();
        }
        let mut keys: Vec<String> = Vec::new();
        if let Some(env) = settings.get("env").and_then(|v| v.as_object()) {
            keys.extend(env.keys().cloned());
        }
        let fallback: &[&str] = if agent_type == "claude_code" {
            &["ANTHROPIC_BASE_URL", "ANTHROPIC_AUTH_TOKEN"]
        } else {
            &["GOOGLE_GEMINI_BASE_URL", "GEMINI_API_KEY"]
        };
        for k in fallback {
            if settings.get(*k).is_some() && !keys.iter().any(|x| x == k) {
                keys.push(k.to_string());
            }
        }
        keys
    }

    /// 把 preset settings_config 注入网关信息后投影到 live（原子写 + 快照回滚）。
    /// 供 switch / 编辑激活预设复用。
    async fn write_live_projection(
        agent_type: &str,
        settings: &serde_json::Value,
        remove_keys: &[String],
    ) -> Result<(), ServiceError> {
        let writer = crate::application::harness::writer_for(agent_type).ok_or_else(|| {
            ServiceError::BadRequest {
                message: format!("该 Agent 类型 ({agent_type}) 不支持配置自动写入"),
                code: None,
            }
        })?;
        let mut effective = settings.clone();
        if let Ok(gs) = crate::models::GatewaySettings::load(
            crate::get_settings_path().ok_or_else(|| ServiceError::Internal {
                message: "无法获取设置路径".to_string(),
                detail: None,
            })?,
        ) {
            let base_url = format!("http://{}:{}/v1", gs.bind_host, gs.bind_port);
            let api_key = crate::application::gateway_key_service::builtin_key_value();
            Self::inject_gateway_config(agent_type, &mut effective, &base_url, &api_key);
        }
        let home = crate::get_home_dir().to_path_buf();
        writer
            .write_live(&home, &effective, remove_keys)
            .await
            .map_err(|e| ServiceError::Internal {
                message: format!("写入 live 配置失败: {e}"),
                detail: None,
            })?;
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
            // CodexWriter 将 endpoint/key 投影到活动 provider 表。
            "codex" | "hermes" => {
                obj.entry("base_url".to_string())
                    .or_insert_with(|| serde_json::json!(base_url));
                obj.entry("api_key".to_string())
                    .or_insert_with(|| serde_json::json!(api_key));
            }
            // OpenCode 使用 options.baseURL/options.apiKey，不识别顶层字段。
            "opencode" => {
                let options = obj
                    .entry("options".to_string())
                    .or_insert_with(|| serde_json::json!({}));
                let options = options
                    .as_object_mut()
                    .expect("options 由 JSON 对象初始化");
                options
                    .entry("baseURL".to_string())
                    .or_insert_with(|| serde_json::json!(base_url));
                options
                    .entry("apiKey".to_string())
                    .or_insert_with(|| serde_json::json!(api_key));
            }
            _ => {}
        }
    }
}

/// 新建预设的默认填充值：silk 网关 base_url/api_key（按 harness 映射到表单字段）。
/// 用户可手动修改。
#[derive(Debug, Clone, serde::Serialize)]
pub struct PresetDefaults {
    pub agent_type: String,
    /// 表单字段 key → 默认值（与前端 harnessForms.ts 的字段 key 对齐）
    pub values: serde_json::Map<String, serde_json::Value>,
}

impl PresetService {
    /// 返回该 harness 的默认表单值（silk 网关端点 + 内置 key）。
    /// 无端点/key 表单字段的 harness（如 codex）返回空 values。
    pub async fn get_defaults(agent_type: String) -> Result<PresetDefaults, ServiceError> {
        validate_non_empty("agent_type", &agent_type)?;
        let mut values = serde_json::Map::new();

        // 读网关设置构造 base_url；设置缺失时回退默认 127.0.0.1:1877
        let settings_path = crate::get_settings_path().unwrap_or(std::path::Path::new(""));
        let (base_url, api_key) = match crate::models::GatewaySettings::load(settings_path) {
            Ok(s) => (
                format!("http://{}:{}/v1", s.bind_host, s.bind_port),
                crate::application::gateway_key_service::builtin_key_value(),
            ),
            Err(_) => (
                "http://127.0.0.1:1877/v1".to_string(),
                crate::application::gateway_key_service::builtin_key_value(),
            ),
        };

        match agent_type.as_str() {
            // Claude Code：env 子对象键（与 harnessForms claudeSpec 字段 key 对齐）
            "claude_code" => {
                values.insert("ANTHROPIC_BASE_URL".into(), base_url.into());
                values.insert("ANTHROPIC_AUTH_TOKEN".into(), api_key.into());
            }
            // OpenCode：options 内键（表单字段 baseURL/apiKey）
            "opencode" => {
                values.insert("baseURL".into(), base_url.into());
                values.insert("apiKey".into(), api_key.into());
            }
            // Hermes：provider 条目内键
            "hermes" => {
                values.insert("base_url".into(), base_url.into());
                values.insert("api_key".into(), api_key.into());
            }
            // Gemini CLI：env 子对象键
            "gemini_cli" => {
                values.insert("GOOGLE_GEMINI_BASE_URL".into(), base_url.into());
                values.insert("GEMINI_API_KEY".into(), api_key.into());
            }
            // codex 等无端点/key 表单字段 → 空
            _ => {}
        }

        tracing::debug!(
            "[presets:defaults] agent_type={agent_type} 默认值字段数={}",
            values.len()
        );
        Ok(PresetDefaults { agent_type, values })
    }
}

// ---------------------------------------------------------------------------
// 行映射
// ---------------------------------------------------------------------------

/// 剥离 live 配置中 silk 注入的字段（backfill 用）：
/// - claude_code/gemini_cli：剥离 env 内的网关键 + _silk_managed
/// - codex/opencode/hermes：剥离顶层 base_url/api_key + _silk_managed

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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn imports_existing_opencode_providers_into_presets() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.expect("memory database");
        sqlx::migrate!("./migrations").run(&pool).await.expect("schema");
        let home = std::env::temp_dir().join(format!("silk-startup-import-{}", uuid::Uuid::new_v4()));
        let config_path = home.join(".config").join("opencode").join("opencode.json");
        std::fs::create_dir_all(config_path.parent().expect("config parent")).expect("config directory");
        std::fs::write(&config_path, r#"{"provider":{"relay":{"npm":"@ai-sdk/openai-compatible","options":{"baseURL":"https://relay.example"}},"other":{"npm":"@ai-sdk/openai"}}}"#).expect("config file");

        PresetService::import_existing_live_configs(&pool, &home).await.expect("startup import");
        PresetService::import_existing_live_configs(&pool, &home).await.expect("startup import repeats safely");
        let rows = sqlx::query_as::<_, (String, String, i64)>("SELECT agent_type, settings_config, is_active FROM presets ORDER BY name")
            .fetch_all(&pool).await.expect("imported presets");
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| row.0 == "opencode"));
        assert!(rows.iter().any(|row| row.1.contains("relay")));
        assert_eq!(rows.iter().filter(|row| row.2 == 1).count(), 0);

        let _ = std::fs::remove_dir_all(home);
    }
}
