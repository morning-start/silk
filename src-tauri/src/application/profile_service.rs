use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::application::config_writer::{self, MergeStrategy, ConfigFormat, LiveSnapshot};
use crate::error::{require_db, require_found, validate_non_empty, ServiceError};
use crate::models::{agent_type::AgentType, NewProfile};
use crate::persistence::common_config_snippet_repo::CommonConfigSnippetRepo;
use crate::persistence::ProfileRepo;

// ---------------------------------------------------------------------------
// AgentConfigWriter trait — 各 Agent 配置写入器接口
// ---------------------------------------------------------------------------

#[async_trait]
pub trait AgentConfigWriter: Send + Sync {
    fn agent_type(&self) -> &'static str;

    fn live_path(&self, home: &Path) -> PathBuf;

    fn merge_strategy(&self) -> MergeStrategy;

    async fn read_live(&self, home: &Path) -> Result<Option<Vec<u8>>, String> {
        let path = self.live_path(home);
        if !path.exists() {
            return Ok(None);
        }
        tokio::fs::read(&path).await.map(Some).map_err(|e| e.to_string())
    }

    async fn write_live(
        &self,
        home: &Path,
        profile_config: &serde_json::Value,
    ) -> Result<(), String> {
        let path = self.live_path(home);
        let _snapshot = LiveSnapshot::take(&path).map_err(|e| format!("备份失败: {e}"))?;
        let mut live = config_writer::read_to_value_async(&path).await?;

        config_writer::json_deep_merge(&mut live, profile_config);
        if !matches!(
            self.merge_strategy(),
            MergeStrategy::IntoProvider | MergeStrategy::IntoHermesProvider
        ) {
            if let Some(obj) = live.as_object_mut() {
                obj.insert("_silk_managed".to_string(), serde_json::json!(true));
            }
        }

        if let Err(e) = config_writer::write_from_value_async(&path, &live).await {
            let _ = _snapshot.restore();
            return Err(e);
        }
        Ok(())
    }

    async fn remove_live(&self, home: &Path, _profile_id: &str) -> Result<(), String> {
        let path = self.live_path(home);
        let _snapshot = LiveSnapshot::take(&path).map_err(|e| format!("备份失败: {e}"))?;
        let empty = serde_json::json!({});
        let mut live = config_writer::read_to_value_async(&path).await?;
        config_writer::json_deep_merge(&mut live, &empty);
        config_writer::write_from_value_async(&path, &live).await
    }

    async fn is_managed(&self, home: &Path) -> Result<bool, String> {
        if let Some(data) = self.read_live(home).await? {
            if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&data) {
                return Ok(val.get("_silk_managed").and_then(|v| v.as_bool()).unwrap_or(false));
            }
        }
        Ok(false)
    }
}

// ---------------------------------------------------------------------------
// 各 Agent 专有 Writer
// ---------------------------------------------------------------------------

// --- Claude Code ---
pub struct ClaudeCodeWriter;

impl Default for ClaudeCodeWriter {
    fn default() -> Self {
        Self
    }
}

impl ClaudeCodeWriter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl AgentConfigWriter for ClaudeCodeWriter {
    fn agent_type(&self) -> &'static str {
        "claude_code"
    }

    fn live_path(&self, home: &Path) -> PathBuf {
        home.join(".claude/settings.json")
    }

    fn merge_strategy(&self) -> MergeStrategy {
        MergeStrategy::TopLevel
    }
}

// --- Codex ---
pub struct CodexWriter;

impl Default for CodexWriter {
    fn default() -> Self {
        Self
    }
}

impl CodexWriter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl AgentConfigWriter for CodexWriter {
    fn agent_type(&self) -> &'static str {
        "codex"
    }

    fn live_path(&self, home: &Path) -> PathBuf {
        home.join(".codex/config.toml")
    }

    fn merge_strategy(&self) -> MergeStrategy {
        MergeStrategy::TopLevel
    }

    async fn write_live(
        &self,
        home: &Path,
        profile_config: &serde_json::Value,
    ) -> Result<(), String> {
        let path = self.live_path(home);
        let _snapshot = LiveSnapshot::take(&path).map_err(|e| format!("备份失败: {e}"))?;

        let mut live = config_writer::read_to_value_async(&path).await?;
        if !live.is_object() {
            live = serde_json::json!({});
        }

        // MCP 服务器：从 profile 中剥离并合并到 [mcp_servers] 顶层表
        let mut profile_clone = profile_config.clone();
        config_writer::apply_mcp_servers(&mut live, &mut profile_clone, "mcp_servers");

        let cfg = profile_clone.as_object().cloned().unwrap_or_default();
        let provider_key = cfg
            .get("model_provider")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("custom")
            .to_string();
        let base_url = cfg
            .get("base_url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let api_key = cfg
            .get("api_key")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let wire_api = cfg
            .get("wire_api")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // 顶层 model 保留（Codex 读取顶层 model 作为当前模型）
        if let Some(m) = cfg.get("model") {
            live["model"] = m.clone();
        }

        match provider_key.as_str() {
            // 内置 openai 的改址走正统机制：顶层 openai_base_url（0.148+ 禁止
            // 建 [model_providers.openai] 表覆盖内置 provider）
            "openai" => {
                if let Some(b) = base_url {
                    live["openai_base_url"] = serde_json::json!(b);
                }
            }
            // 这两个保留 id 没有等价顶层旋钮，建表会导致 Codex 拒绝加载整份配置
            "ollama" | "lmstudio" => {
                return Err(format!(
                    "Codex 禁止覆盖内置 provider `{provider_key}`（0.148 起会拒绝加载整份配置）；请改用自定义 provider id"
                ));
            }
            key => {
                // 确保 [model_providers.<key>] 表存在，且 name 非空（0.149 起为空整份拒载）
                let live_obj = live
                    .as_object_mut()
                    .ok_or_else(|| "config.toml 顶层不是对象".to_string())?;
                let providers = live_obj
                    .entry("model_providers".to_string())
                    .or_insert_with(|| serde_json::json!({}));
                let table = providers
                    .as_object_mut()
                    .ok_or_else(|| "config.toml 的 model_providers 不是表，无法写入".to_string())?;
                let entry = table
                    .entry(key.to_string())
                    .or_insert_with(|| serde_json::json!({}));
                if let Some(e) = entry.as_object_mut() {
                    let has_name = e
                        .get("name")
                        .and_then(|v| v.as_str())
                        .map(str::trim)
                        .filter(|n| !n.is_empty())
                        .is_some();
                    if !has_name {
                        e.insert("name".to_string(), serde_json::json!(key));
                    }
                    if let Some(b) = &base_url {
                        e.insert("base_url".to_string(), serde_json::json!(b));
                    }
                    if let Some(k) = &api_key {
                        e.insert("api_key".to_string(), serde_json::json!(k));
                    }
                    if let Some(w) = &wire_api {
                        e.insert("wire_api".to_string(), serde_json::json!(w));
                    }
                }
                live["model_provider"] = serde_json::json!(key);
            }
        }

        if let Some(obj) = live.as_object_mut() {
            obj.insert("_silk_managed".to_string(), serde_json::json!(true));
        }

        // TOML 原生 mcp_servers 键透传（前端 Codex 表单以 [mcp_servers.<name>] 写入）
        if let Some(mcp) = cfg.get("mcp_servers") {
            live["mcp_servers"] = mcp.clone();
        }

        if let Err(e) = config_writer::write_from_value_async(&path, &live).await {
            let _ = _snapshot.restore();
            return Err(e);
        }
        Ok(())
    }
}

// --- Gemini CLI ---
pub struct GeminiCliWriter;

impl Default for GeminiCliWriter {
    fn default() -> Self {
        Self
    }
}

impl GeminiCliWriter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl AgentConfigWriter for GeminiCliWriter {
    fn agent_type(&self) -> &'static str {
        "gemini_cli"
    }

    fn live_path(&self, home: &Path) -> PathBuf {
        home.join(".gemini/settings.json")
    }

    fn merge_strategy(&self) -> MergeStrategy {
        MergeStrategy::IntoSubObject("env")
    }

    async fn write_live(
        &self,
        home: &Path,
        profile_config: &serde_json::Value,
    ) -> Result<(), String> {
        let path = self.live_path(home);
        config_writer::merge_into_live_async_with_mcp(
            &path,
            profile_config,
            MergeStrategy::IntoSubObject("env"),
            Some("mcpServers"),
        )
        .await
    }
}

// --- OpenCode（累加模式）---
pub struct OpenCodeWriter;

impl Default for OpenCodeWriter {
    fn default() -> Self {
        Self
    }
}

impl OpenCodeWriter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl AgentConfigWriter for OpenCodeWriter {
    fn agent_type(&self) -> &'static str {
        "opencode"
    }

    fn live_path(&self, home: &Path) -> PathBuf {
        home.join(".config/opencode/opencode.json")
    }

    fn merge_strategy(&self) -> MergeStrategy {
        MergeStrategy::IntoProvider
    }

    async fn write_live(
        &self,
        home: &Path,
        profile_config: &serde_json::Value,
    ) -> Result<(), String> {
        let path = self.live_path(home);
        config_writer::merge_into_live_async_with_mcp(
            &path,
            profile_config,
            MergeStrategy::IntoProvider,
            Some("mcp"),
        )
        .await
    }

    async fn remove_live(&self, home: &Path, profile_id: &str) -> Result<(), String> {
        let path = self.live_path(home);
        config_writer::remove_provider_from_live_async(&path, profile_id, MergeStrategy::IntoProvider)
            .await
    }

    async fn is_managed(&self, home: &Path) -> Result<bool, String> {
        let path = self.live_path(home);
        if !path.exists() {
            return Ok(false);
        }
        let val = config_writer::read_to_value_async(&path).await?;
        // 检查是否有任何 provider 被 silk 管理
        if let Some(providers) = val.get("provider").and_then(|v| v.as_object()) {
            for (_id, provider) in providers {
                if provider.get("_silk_managed").and_then(|v| v.as_bool()).unwrap_or(false) {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}

// --- Hermes（累加模式，YAML）---
pub struct HermesWriter;

impl Default for HermesWriter {
    fn default() -> Self {
        Self
    }
}

impl HermesWriter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl AgentConfigWriter for HermesWriter {
    fn agent_type(&self) -> &'static str {
        "hermes"
    }

    fn live_path(&self, home: &Path) -> PathBuf {
        home.join(".hermes/config.yaml")
    }

    fn merge_strategy(&self) -> MergeStrategy {
        MergeStrategy::IntoHermesProvider
    }

    async fn write_live(
        &self,
        home: &Path,
        profile_config: &serde_json::Value,
    ) -> Result<(), String> {
        let path = self.live_path(home);
        config_writer::merge_into_live_async_with_mcp(
            &path,
            profile_config,
            MergeStrategy::IntoHermesProvider,
            Some("mcp_servers"),
        )
        .await
    }

    async fn remove_live(&self, home: &Path, profile_id: &str) -> Result<(), String> {
        let path = self.live_path(home);
        config_writer::remove_provider_from_live_async(&path, profile_id, MergeStrategy::IntoHermesProvider)
            .await
    }

    async fn is_managed(&self, home: &Path) -> Result<bool, String> {
        let path = self.live_path(home);
        if !path.exists() {
            return Ok(false);
        }
        let val = config_writer::read_to_value_async(&path).await?;
        // 检查 custom_providers 列表是否有任何 provider 被 silk 管理
        if let Some(list) = val.get("custom_providers").and_then(|v| v.as_array()) {
            for provider in list {
                if provider.get("_silk_managed").and_then(|v| v.as_bool()).unwrap_or(false) {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}

// ---------------------------------------------------------------------------
// Writer 注册表
// ---------------------------------------------------------------------------

fn builtin_writers() -> Vec<Box<dyn AgentConfigWriter>> {
    vec![
        Box::new(ClaudeCodeWriter::new()),
        Box::new(CodexWriter::new()),
        Box::new(GeminiCliWriter::new()),
        Box::new(OpenCodeWriter::new()),
        Box::new(HermesWriter::new()),
    ]
}

fn writer_for(agent_type: &str) -> Option<Box<dyn AgentConfigWriter>> {
    builtin_writers().into_iter().find(|w| w.agent_type() == agent_type)
}

// ---------------------------------------------------------------------------
// 校验
// ---------------------------------------------------------------------------

fn config_format_for(agent_type: &str) -> ConfigFormat {
    match agent_type {
        "codex" => ConfigFormat::Toml,
        "hermes" => ConfigFormat::Yaml,
        _ => ConfigFormat::Json,
    }
}

fn validate_profile_payload(agent_type: &str, config_json: &str) -> Result<(), ServiceError> {
    validate_non_empty("agent_type", agent_type)?;
    validate_non_empty("config_json", config_json)?;

    if !AgentType::is_valid(agent_type) {
        return Err(ServiceError::BadRequest {
            message: format!("不支持的 agent_type: {}", agent_type),
            code: None,
        });
    }

    let fmt = config_format_for(agent_type);
    config_writer::validate_config_text(config_json, fmt).map_err(|e| {
        ServiceError::BadRequest {
            message: format!("config_json 格式错误: {e}"),
            code: None,
        }
    })?;

    validate_agent_schema(agent_type, config_json, fmt)?;

    Ok(())
}

/// 按 agent 校验 config_json 的必填字段（在格式校验之后执行）
fn validate_agent_schema(
    agent_type: &str,
    config_json: &str,
    fmt: ConfigFormat,
) -> Result<(), ServiceError> {
    let value: serde_json::Value = match fmt {
        ConfigFormat::Json => serde_json::from_str(config_json).map_err(|e| {
            ServiceError::BadRequest {
                message: format!("config_json 解析失败: {e}"),
                code: None,
            }
        })?,
        ConfigFormat::Toml => toml::from_str(config_json).map_err(|e| {
            ServiceError::BadRequest {
                message: format!("config_json 解析失败: {e}"),
                code: None,
            }
        })?,
        ConfigFormat::Yaml => serde_yaml::from_str(config_json).map_err(|e| {
            ServiceError::BadRequest {
                message: format!("config_json 解析失败: {e}"),
                code: None,
            }
        })?,
    };

    let bad = |message: String| ServiceError::BadRequest { message, code: None };

    match agent_type {
        // Codex：必填 model / wire_api（顶层字符串）
        "codex" => {
            let mut missing = Vec::new();
            for field in ["model", "wire_api"] {
                let ok = value
                    .get(field)
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .is_some_and(|s| !s.is_empty());
                if !ok {
                    missing.push(field);
                }
            }
            if !missing.is_empty() {
                return Err(bad(format!("codex 配置缺少必填字段: {}", missing.join(", "))));
            }
        }
        // Gemini CLI：至少一个 env 键（config_json 即 env 内容）
        "gemini_cli" => {
            if value.as_object().is_none_or(|o| o.is_empty()) {
                return Err(bad("gemini_cli 配置至少需要一个 env 键（如 GEMINI_MODEL）".to_string()));
            }
        }
        // OpenCode / Hermes：必填 provider 标识（_silk_provider_id）
        "opencode" | "hermes" => {
            let has_id = value
                .get("_silk_provider_id")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .is_some_and(|s| !s.is_empty());
            if !has_id {
                return Err(bad(format!("{agent_type} 配置缺少必填字段: _silk_provider_id")));
            }
        }
        _ => {}
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// CRUD
// ---------------------------------------------------------------------------

pub async fn list(agent_type: String) -> Result<Vec<ProfileResponse>, ServiceError> {
    let pool = require_db()?;
    let profiles = ProfileRepo::find_by_agent_type(pool, &agent_type).await?;
    Ok(profiles.into_iter().map(ProfileResponse::from).collect())
}

pub async fn get(profile_id: String) -> Result<ProfileResponse, ServiceError> {
    let pool = require_db()?;
    let profile = require_found(
        ProfileRepo::find_by_id(pool, &profile_id).await?,
        "Profile",
    )?;
    Ok(ProfileResponse::from(profile))
}

pub async fn create(payload: CreateProfilePayload) -> Result<ProfileResponse, ServiceError> {
    validate_profile_payload(&payload.agent_type, &payload.config_json)?;

    let pool = require_db()?;
    let new = NewProfile {
        name: payload.name.trim().to_string(),
        agent_type: payload.agent_type,
        config_json: payload.config_json,
        is_active: Some(false),
        sort_index: payload.sort_index,
    };
    let profile = ProfileRepo::create(pool, &new).await?;
    Ok(ProfileResponse::from(profile))
}

pub async fn update(
    profile_id: String,
    payload: UpdateProfilePayload,
) -> Result<ProfileResponse, ServiceError> {
    let pool = require_db()?;

    let _existing = require_found(
        ProfileRepo::find_by_id(pool, &profile_id).await?,
        "Profile",
    )?;

    if let Some(ref config_json) = payload.config_json {
        let agent_type = _existing.agent_type.as_str();
        let fmt = config_format_for(agent_type);
        config_writer::validate_config_text(config_json, fmt).map_err(|e| {
            ServiceError::BadRequest {
                message: format!("config_json 格式错误: {e}"),
                code: None,
            }
        })?;
    }

    let update = crate::models::UpdateProfile {
        name: payload.name.map(|n| n.trim().to_string()),
        config_json: payload.config_json,
        is_active: None,
        sort_index: payload.sort_index,
    };

    let profile = require_found(
        ProfileRepo::update(pool, &profile_id, &update).await?,
        "Profile",
    )?;
    Ok(ProfileResponse::from(profile))
}

pub async fn delete(profile_id: String) -> Result<bool, ServiceError> {
    let pool = require_db()?;
    ProfileRepo::delete(pool, &profile_id).await.map_err(ServiceError::from)
}

// ---------------------------------------------------------------------------
// 切换（核心）
// ---------------------------------------------------------------------------

pub async fn switch(
    agent_type: String,
    profile_id: String,
) -> Result<SwitchResult, ServiceError> {
    let pool = require_db()?;

    let profile = require_found(
        ProfileRepo::find_by_id(pool, &profile_id).await?,
        "Profile",
    )?;

    if profile.agent_type != agent_type {
        return Err(ServiceError::BadRequest {
            message: format!("Profile 的 agent_type ({}) 与请求 ({}) 不匹配", profile.agent_type, agent_type),
            code: None,
        });
    }

    let mut warnings = Vec::new();

    let writer = match writer_for(&agent_type) {
        Some(w) => w,
        None => {
            return switch_db_only(pool, &agent_type, &profile_id).await;
        }
    };

    let fmt = config_format_for(&agent_type);
    let config: serde_json::Value = match fmt {
        ConfigFormat::Json => serde_json::from_str(&profile.config_json).map_err(|e| {
            ServiceError::BadRequest {
                message: format!("config_json 解析失败: {e}"),
                code: None,
            }
        })?,
        ConfigFormat::Toml => toml::from_str(&profile.config_json).map_err(|e| {
            ServiceError::BadRequest {
                message: format!("config_json 解析失败: {e}"),
                code: None,
            }
        })?,
        ConfigFormat::Yaml => serde_yaml::from_str(&profile.config_json).map_err(|e| {
            ServiceError::BadRequest {
                message: format!("config_json 解析失败: {e}"),
                code: None,
            }
        })?,
    };

    let _snippet = CommonConfigSnippetRepo::find_by_agent(pool, &agent_type).await?;

    let mut effective_config = build_effective_config(&config, _snippet.as_ref());

    let home = crate::get_home_dir().to_path_buf();

    // 注入 base_url + api_key（按 harness 映射到正确字段位置）
    if let Ok(settings) = crate::models::GatewaySettings::load(
        crate::get_settings_path().ok_or_else(|| ServiceError::Internal {
            message: "无法获取设置路径".to_string(),
            detail: None,
        })?,
    ) {
        let base_url = format!("http://{}:{}/v1", settings.bind_host, settings.bind_port);
        let api_key = crate::application::gateway_key_service::builtin_key_value();
        inject_gateway_config(&agent_type, &mut effective_config, &base_url, &api_key);
    }

    // 冲突检测：live 配置存在但未被 silk 管理 → 提示外部修改
    if let Some(conflict) = detect_live_conflict(writer.as_ref(), &home)
        .await
        .map_err(|e| ServiceError::Internal {
            message: format!("检查 live 配置冲突失败: {e}"),
            detail: None,
        })?
    {
        warnings.push(conflict);
    }

    // 网关联动：校验 profile 引用的模型在模型池存在
    if let Ok(models) = crate::application::models_listing::list_all_models().await {
        let mut missing = missing_referenced_models(&agent_type, &config, &models);
        if !missing.is_empty() {
            warnings.push(format!(
                "以下模型在模型池中不存在，可能无法路由: {}",
                missing.join(", ")
            ));
        }
    }

    // 写入前备份 live 配置；写入失败时恢复原状并回滚 DB 状态
    let snapshot = config_writer::LiveSnapshot::take(&writer.live_path(&home))
        .map_err(|e| ServiceError::Internal {
            message: format!("备份 live 配置失败: {e}"),
            detail: None,
        })?;

    if let Err(e) = writer.write_live(&home, &effective_config).await {
        let _ = snapshot.restore();
        return Err(ServiceError::Internal {
            message: format!("写入 live 配置失败，已回滚: {e}"),
            detail: None,
        });
    }

    ProfileRepo::deactivate_all(pool, &agent_type).await?;
    ProfileRepo::activate(pool, &profile_id).await?;

    let requires_restart = !matches!(agent_type.as_str(), "opencode" | "hermes");

    if requires_restart {
        warnings.push("请重启终端/应用以使配置生效".to_string());
    }

    Ok(SwitchResult {
        success: true,
        warnings,
        requires_restart,
    })
}

async fn switch_db_only(
    pool: &sqlx::SqlitePool,
    agent_type: &str,
    profile_id: &str,
) -> Result<SwitchResult, ServiceError> {
    ProfileRepo::deactivate_all(pool, agent_type).await?;
    ProfileRepo::activate(pool, profile_id).await?;

    Ok(SwitchResult {
        success: true,
        warnings: vec!["该 Agent 类型尚未支持配置自动写入，请手动配置".to_string()],
        requires_restart: true,
    })
}

/// 按 harness 把网关 base_url/api_key 注入到 effective_config 的正确字段位置。
/// 同时保留 profile 自定义的 env 键（仅添加缺失项，不覆盖已存在键）。
fn inject_gateway_config(
    agent_type: &str,
    effective_config: &mut serde_json::Value,
    base_url: &str,
    api_key: &str,
) {
    let Some(obj) = effective_config.as_object_mut() else {
        return;
    };
    match agent_type {
        // Claude Code：写入 env 子对象（settings.json 的 env 作为环境变量注入）
        "claude_code" => {
            let env = obj
                .entry("env".to_string())
                .or_insert_with(|| serde_json::json!({}));
            if let Some(e) = env.as_object_mut() {
                e.entry("ANTHROPIC_BASE_URL".to_string())
                    .or_insert_with(|| serde_json::json!(base_url));
                e.entry("ANTHROPIC_AUTH_TOKEN".to_string())
                    .or_insert_with(|| serde_json::json!(api_key));
            }
        }
        // Gemini CLI：profile_config 即 env 内容（IntoSubObject("env") 合并），
        // 注入 GOOGLE_GEMINI_BASE_URL / GEMINI_API_KEY
        "gemini_cli" => {
            obj.entry("GOOGLE_GEMINI_BASE_URL".to_string())
                .or_insert_with(|| serde_json::json!(base_url));
            obj.entry("GEMINI_API_KEY".to_string())
                .or_insert_with(|| serde_json::json!(api_key));
        }
        // Codex：顶层注入，CodexWriter 会搬进 [model_providers.<id>] 表；
        // OpenCode / Hermes：整个 profile_config 即 provider 条目内容，顶层注入即条目内
        _ => {
            obj.entry("base_url".to_string())
                .or_insert_with(|| serde_json::json!(base_url));
            obj.entry("api_key".to_string())
                .or_insert_with(|| serde_json::json!(api_key));
        }
    }
}

/// 冲突检测：live 配置存在但未被 silk 管理 → 返回外部修改提示。
async fn detect_live_conflict(
    writer: &dyn AgentConfigWriter,
    home: &Path,
) -> Result<Option<String>, String> {
    if !writer.live_path(home).exists() {
        return Ok(None);
    }
    let managed = writer.is_managed(home).await?;
    if managed {
        Ok(None)
    } else {
        Ok(Some(format!(
            "检测到 {} 的 live 配置未被 silk 管理，切换将覆盖外部修改",
            writer.agent_type()
        )))
    }
}

/// 提取 profile config 中引用的模型 id（按 agent 结构）
fn referenced_models(agent_type: &str, config: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();
    match agent_type {
        "claude_code" => {
            if let Some(roles) = config.get("roles").and_then(|v| v.as_object()) {
                for v in roles.values() {
                    if let Some(s) = v.as_str() {
                        out.push(s.to_string());
                    }
                }
            }
        }
        "gemini_cli" => {
            for key in ["GEMINI_MODEL", "model"] {
                if let Some(s) = config.get(key).and_then(|v| v.as_str()) {
                    out.push(s.to_string());
                }
            }
        }
        "codex" => {
            if let Some(s) = config.get("model").and_then(|v| v.as_str()) {
                out.push(s.to_string());
            }
        }
        "opencode" => {
            if let Some(enabled) = config.get("enabled_models").and_then(|v| v.as_object()) {
                for list in enabled.values() {
                    if let Some(arr) = list.as_array() {
                        for m in arr {
                            if let Some(s) = m.as_str() {
                                out.push(s.to_string());
                            }
                        }
                    }
                }
            }
        }
        "hermes" => {
            if let Some(models) = config.get("models").and_then(|v| v.as_object()) {
                out.extend(models.keys().cloned());
            }
        }
        _ => {}
    }
    out
}

/// 网关联动：返回 profile 引用的、但模型池中不存在的模型 id。
fn missing_referenced_models(
    agent_type: &str,
    config: &serde_json::Value,
    models: &[crate::application::models_listing::ModelListingItem],
) -> Vec<String> {
    let known: std::collections::HashSet<&str> = models
        .iter()
        .flat_map(|m| {
            let mut keys = vec![m.id.as_str()];
            if let Some(mid) = &m.model_mapping_id {
                keys.push(mid.as_str());
            }
            keys
        })
        .collect();
    referenced_models(agent_type, config)
        .into_iter()
        .filter(|m| !known.contains(m.as_str()))
        .collect()
}

// ---------------------------------------------------------------------------
// live 配置导入（反向能力：现有配置 → Profile）
// ---------------------------------------------------------------------------

/// 从 live 配置文件导入为 Profile（未激活）。
///
/// 读取 agent 的 live 配置 → 剥离 silk 注入的字段 → 按该 agent 的格式序列化为
/// config_json → 创建 Profile。live 文件不存在时返回明确错误。
pub async fn import_live_config(agent_type: String) -> Result<ProfileResponse, ServiceError> {
    validate_non_empty("agent_type", &agent_type)?;
    if !AgentType::is_valid(&agent_type) {
        return Err(ServiceError::BadRequest {
            message: format!("不支持的 agent_type: {}", agent_type),
            code: None,
        });
    }

    let writer = writer_for(&agent_type).ok_or_else(|| ServiceError::BadRequest {
        message: format!("该 Agent 类型 ({agent_type}) 不支持配置自动写入，无法导入"),
        code: None,
    })?;

    let home = crate::get_home_dir().to_path_buf();
    let data = writer
        .read_live(&home)
        .await
        .map_err(|e| ServiceError::Internal {
            message: format!("读取 live 配置失败: {e}"),
            detail: None,
        })?
        .ok_or_else(|| ServiceError::BadRequest {
            message: format!("未找到 {} 的 live 配置（{}）", agent_type, writer.live_path(&home).display()),
            code: None,
        })?;

    let text = String::from_utf8(data).map_err(|_| ServiceError::BadRequest {
        message: "live 配置不是合法 UTF-8 文本".to_string(),
        code: None,
    })?;

    let fmt = config_format_for(&agent_type);
    config_writer::validate_config_text(&text, fmt).map_err(|e| {
        ServiceError::BadRequest {
            message: format!("live 配置格式错误: {e}"),
            code: None,
        }
    })?;

    let cleaned = clean_imported_config(&agent_type, &text, fmt)?;
    validate_profile_payload(&agent_type, &cleaned)?;

    let pool = require_db()?;
    let name = format!("导入 {}", AgentType::name_for(&agent_type).unwrap_or(&agent_type));
    let new = NewProfile {
        name,
        agent_type,
        config_json: cleaned,
        is_active: Some(false),
        sort_index: None,
    };
    let profile = ProfileRepo::create(pool, &new).await?;
    Ok(ProfileResponse::from(profile))
}

/// 剥离 live 配置中 silk 注入的字段，还原为用户可编辑的 config_json。
fn clean_imported_config(
    agent_type: &str,
    text: &str,
    fmt: ConfigFormat,
) -> Result<String, ServiceError> {
    let mut value: serde_json::Value = match fmt {
        ConfigFormat::Json => serde_json::from_str(text).map_err(|e| {
            ServiceError::BadRequest { message: format!("live 配置解析失败: {e}"), code: None }
        })?,
        ConfigFormat::Toml => toml::from_str(text).map_err(|e| {
            ServiceError::BadRequest { message: format!("live 配置解析失败: {e}"), code: None }
        })?,
        ConfigFormat::Yaml => serde_yaml::from_str(text).map_err(|e| {
            ServiceError::BadRequest { message: format!("live 配置解析失败: {e}"), code: None }
        })?,
    };

    match agent_type {
        // Claude Code：保留 roles，剥离注入的 env（ANTHROPIC_BASE_URL/AUTH_TOKEN）与接管标记
        "claude_code" => {
            if let Some(obj) = value.as_object_mut() {
                obj.remove("env");
                obj.remove("_silk_managed");
                obj.remove("base_url");
                obj.remove("api_key");
            }
        }
        // Gemini CLI：config_json 即 env 内容，剥离注入的网关键
        "gemini_cli" => {
            if let Some(obj) = value.as_object_mut() {
                obj.remove("GOOGLE_GEMINI_BASE_URL");
                obj.remove("GEMINI_API_KEY");
                obj.remove("_silk_managed");
            }
        }
        // Codex：保留顶层 model/model_provider/wire_api，剥离注入字段与 model_providers 表
        "codex" => {
            if let Some(obj) = value.as_object_mut() {
                obj.remove("base_url");
                obj.remove("api_key");
                obj.remove("openai_base_url");
                obj.remove("_silk_managed");
                obj.remove("model_providers");
            }
        }
        // OpenCode：从 provider.<id> 提取第一个条目为 profile 配置
        "opencode" => {
            if let Some(providers) = value.get("provider").and_then(|v| v.as_object()) {
                if let Some((id, entry)) = providers.iter().next() {
                    let mut cfg = entry.clone();
                    if let Some(o) = cfg.as_object_mut() {
                        o.remove("_silk_managed");
                        o.insert(
                            "_silk_provider_id".to_string(),
                            serde_json::json!(id),
                        );
                    }
                    value = cfg;
                }
            }
        }
        // Hermes：从 custom_providers 列表提取第一条为 profile 配置
        "hermes" => {
            if let Some(list) = value.get("custom_providers").and_then(|v| v.as_array()) {
                if let Some(entry) = list.iter().next() {
                    let mut cfg = entry.clone();
                    if let Some(o) = cfg.as_object_mut() {
                        o.remove("_silk_managed");
                        if o.get("name").is_none() {
                            o.insert("name".to_string(), serde_json::json!("imported"));
                        }
                    }
                    value = cfg;
                }
            }
        }
        _ => {}
    }

    // _silk_provider_id 若无显式值，用 name 兜底（schema 要求 opencode/hermes 必填）
    if matches!(agent_type, "opencode" | "hermes") {
        let has_id = value
            .get("_silk_provider_id")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .is_some_and(|s| !s.is_empty());
        if !has_id {
            let fallback = value
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("imported")
                .to_string();
            if let Some(o) = value.as_object_mut() {
                o.insert("_silk_provider_id".to_string(), serde_json::json!(fallback));
            }
        }
    }

    match fmt {
        ConfigFormat::Json => serde_json::to_string_pretty(&value).map_err(|e| {
            ServiceError::Internal { message: format!("config_json 序列化失败: {e}"), detail: None }
        }),
        ConfigFormat::Toml => toml::to_string_pretty(&value).map_err(|e| {
            ServiceError::Internal { message: format!("config_json 序列化失败: {e}"), detail: None }
        }),
        ConfigFormat::Yaml => serde_yaml::to_string(&value).map_err(|e| {
            ServiceError::Internal { message: format!("config_json 序列化失败: {e}"), detail: None }
        }),
    }
}

// ---------------------------------------------------------------------------
// live 状态查询（三态回显：当前激活 / live 未管理 / 需重启）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentLiveStatus {
    /// live 配置文件是否被 silk 管理（存在 _silk_managed 标记）
    pub managed: bool,
    /// live 配置文件路径
    pub live_path: String,
}

pub async fn get_agent_live_status(agent_type: String) -> Result<AgentLiveStatus, ServiceError> {
    let writer = writer_for(&agent_type).ok_or_else(|| ServiceError::BadRequest {
        message: format!("该 Agent 类型 ({agent_type}) 不支持配置自动写入"),
        code: None,
    })?;
    let home = crate::get_home_dir().to_path_buf();
    let managed = writer.is_managed(&home).await.map_err(|e| ServiceError::Internal {
        message: format!("检查 live 配置状态失败: {e}"),
        detail: None,
    })?;
    Ok(AgentLiveStatus {
        managed,
        live_path: writer.live_path(&home).display().to_string(),
    })
}

// ---------------------------------------------------------------------------
// 通用配置片段
// ---------------------------------------------------------------------------

pub async fn get_common_snippet(
    agent_type: String,
) -> Result<Option<String>, ServiceError> {
    let pool = require_db()?;
    let snippet = CommonConfigSnippetRepo::find_by_agent(pool, &agent_type).await?;
    Ok(snippet.map(|s| s.content))
}

pub async fn set_common_snippet(
    agent_type: String,
    content: String,
) -> Result<(), ServiceError> {
    let pool = require_db()?;
    CommonConfigSnippetRepo::upsert(pool, &agent_type, &content).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 辅助
// ---------------------------------------------------------------------------

fn build_effective_config(
    config: &serde_json::Value,
    snippet: Option<&crate::persistence::CommonConfigSnippet>,
) -> serde_json::Value {
    let mut effective = config.clone();

    if let Some(snippet) = snippet {
        if let Ok(snippet_val) = serde_json::from_str::<serde_json::Value>(&snippet.content) {
            config_writer::json_deep_merge(&mut effective, &snippet_val);
        }
    }

    effective
}

// ---------------------------------------------------------------------------
// Response / Payload 类型
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileResponse {
    pub id: String,
    pub name: String,
    pub agent_type: String,
    pub config_json: String,
    pub is_active: bool,
    pub sort_index: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<crate::models::Profile> for ProfileResponse {
    fn from(p: crate::models::Profile) -> Self {
        Self {
            id: p.id,
            name: p.name,
            agent_type: p.agent_type,
            config_json: p.config_json,
            is_active: p.is_active,
            sort_index: p.sort_index,
            created_at: p.created_at.to_string(),
            updated_at: p.updated_at.to_string(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateProfilePayload {
    pub name: String,
    pub agent_type: String,
    pub config_json: String,
    pub sort_index: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProfilePayload {
    pub name: Option<String>,
    pub config_json: Option<String>,
    pub sort_index: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchResult {
    pub success: bool,
    pub warnings: Vec<String>,
    pub requires_restart: bool,
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_home(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("silk-profile-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    // ---- validate_agent_schema ----

    #[test]
    fn schema_codex_requires_model_and_wire_api() {
        // 缺 model
        let err = validate_agent_schema("codex", "wire_api = \"responses\"\n", ConfigFormat::Toml).unwrap_err();
        assert!(err.to_string().contains("model"));

        // 缺 wire_api
        let err = validate_agent_schema("codex", "model = \"gpt-5\"\n", ConfigFormat::Toml).unwrap_err();
        assert!(err.to_string().contains("wire_api"));

        // 合法
        validate_agent_schema(
            "codex",
            "model = \"gpt-5\"\nmodel_provider = \"custom\"\nwire_api = \"responses\"\n",
            ConfigFormat::Toml,
        )
        .unwrap();
    }

    #[test]
    fn schema_gemini_requires_env_key() {
        let err = validate_agent_schema("gemini_cli", "{}", ConfigFormat::Json).unwrap_err();
        assert!(err.to_string().contains("env 键"));

        validate_agent_schema("gemini_cli", "{\"GEMINI_MODEL\": \"gemini-2.5-pro\"}", ConfigFormat::Json).unwrap();
    }

    #[test]
    fn schema_opencode_hermes_require_provider_id() {
        let err = validate_agent_schema("opencode", "{}", ConfigFormat::Json).unwrap_err();
        assert!(err.to_string().contains("_silk_provider_id"));

        let err = validate_agent_schema(
            "hermes",
            "base_url: http://127.0.0.1:1877/v1\n",
            ConfigFormat::Yaml,
        )
        .unwrap_err();
        assert!(err.to_string().contains("_silk_provider_id"));

        validate_agent_schema(
            "opencode",
            "{\"_silk_provider_id\": \"daily\", \"enabled_models\": {}}",
            ConfigFormat::Json,
        )
        .unwrap();
    }

    // ---- CodexWriter::write_live ----

    #[tokio::test]
    async fn codex_writer_builds_model_providers_table() {
        let home = temp_home("codex-table");
        let writer = CodexWriter::new();
        let cfg = json!({
            "model": "gpt-5",
            "model_provider": "silk",
            "wire_api": "responses",
            "base_url": "http://127.0.0.1:1877/v1",
            "api_key": "sk-silk-test"
        });
        writer.write_live(&home, &cfg).await.unwrap();

        let text = std::fs::read_to_string(home.join(".codex/config.toml")).unwrap();
        let doc: serde_json::Value = toml::from_str(&text).unwrap();
        let entry = &doc["model_providers"]["silk"];
        assert_eq!(entry["name"].as_str(), Some("silk"));
        assert_eq!(entry["base_url"].as_str(), Some("http://127.0.0.1:1877/v1"));
        assert_eq!(entry["wire_api"].as_str(), Some("responses"));
        assert_eq!(doc["model"].as_str(), Some("gpt-5"));
        assert_eq!(doc["model_provider"].as_str(), Some("silk"));
        let _ = std::fs::remove_dir_all(&home);
    }

    #[tokio::test]
    async fn codex_writer_openai_uses_openai_base_url() {
        let home = temp_home("codex-openai");
        let writer = CodexWriter::new();
        let cfg = json!({
            "model": "gpt-5",
            "model_provider": "openai",
            "wire_api": "responses",
            "base_url": "http://127.0.0.1:1877/v1"
        });
        writer.write_live(&home, &cfg).await.unwrap();

        let text = std::fs::read_to_string(home.join(".codex/config.toml")).unwrap();
        let doc: serde_json::Value = toml::from_str(&text).unwrap();
        assert_eq!(doc["openai_base_url"].as_str(), Some("http://127.0.0.1:1877/v1"));
        assert!(doc.get("model_providers").is_none(), "内置 openai 不应建 model_providers 表");
        let _ = std::fs::remove_dir_all(&home);
    }

    #[tokio::test]
    async fn codex_writer_rejects_reserved_ollama() {
        let home = temp_home("codex-ollama");
        let writer = CodexWriter::new();
        let cfg = json!({
            "model": "llama3",
            "model_provider": "ollama",
            "wire_api": "chat",
            "base_url": "http://127.0.0.1:1877/v1"
        });
        let err = writer.write_live(&home, &cfg).await.unwrap_err();
        assert!(err.contains("ollama"));
        // 不应写入任何文件
        assert!(!home.join(".codex/config.toml").exists());
        let _ = std::fs::remove_dir_all(&home);
    }

    // ---- 批次3：env 注入扩展 / 冲突检测 / 网关联动 ----

    #[test]
    fn inject_gateway_config_keeps_custom_env_and_does_not_overwrite() {
        // claude_code：profile 自定义 env 键保留，注入键不覆盖已有值
        let mut cfg = json!({
            "roles": { "sonnet": "gpt-5" },
            "env": { "ANTHROPIC_MODEL": "custom-sonnet", "ANTHROPIC_BASE_URL": "user-provided" }
        });
        inject_gateway_config("claude_code", &mut cfg, "http://127.0.0.1:1877/v1", "sk-silk");
        let env = cfg.get("env").unwrap();
        // 自定义键保留
        assert_eq!(env.get("ANTHROPIC_MODEL").and_then(|v| v.as_str()), Some("custom-sonnet"));
        // 已存在的注入键不被覆盖（用户显式提供优先）
        assert_eq!(env.get("ANTHROPIC_BASE_URL").and_then(|v| v.as_str()), Some("user-provided"));
        // 缺失的注入键被补充
        assert_eq!(env.get("ANTHROPIC_AUTH_TOKEN").and_then(|v| v.as_str()), Some("sk-silk"));

        // gemini_cli：自定义 env 键保留，注入 GOOGLE_* 键
        let mut cfg = json!({ "GEMINI_MODEL": "gemini-2.5-pro" });
        inject_gateway_config("gemini_cli", &mut cfg, "http://127.0.0.1:1877/v1", "sk-silk");
        assert_eq!(cfg.get("GEMINI_MODEL").and_then(|v| v.as_str()), Some("gemini-2.5-pro"));
        assert_eq!(cfg.get("GOOGLE_GEMINI_BASE_URL").and_then(|v| v.as_str()), Some("http://127.0.0.1:1877/v1"));
        assert_eq!(cfg.get("GEMINI_API_KEY").and_then(|v| v.as_str()), Some("sk-silk"));
    }

    #[test]
    fn referenced_models_extracts_per_agent() {
        let claude = json!({ "roles": { "sonnet": "m1", "opus": "m2" } });
        let mut models = referenced_models("claude_code", &claude);
        models.sort();
        assert_eq!(models, vec!["m1", "m2"]);

        let gemini = json!({ "GEMINI_MODEL": "gemini-2.5-pro" });
        assert_eq!(referenced_models("gemini_cli", &gemini), vec!["gemini-2.5-pro"]);

        let opencode = json!({ "enabled_models": { "silk": ["a", "b"], "other": ["c"] } });
        let mut opencode_models = referenced_models("opencode", &opencode);
        opencode_models.sort();
        assert_eq!(opencode_models, vec!["a", "b", "c"]);

        let hermes = json!({ "models": { "llama3": {}, "qwen": {} } });
        assert_eq!(referenced_models("hermes", &hermes), vec!["llama3", "qwen"]);

        let codex = json!({ "model": "gpt-5" });
        assert_eq!(referenced_models("codex", &codex), vec!["gpt-5"]);
    }

    #[test]
    fn missing_referenced_models_flags_unknown() {
        let cfg = json!({ "roles": { "sonnet": "known-model", "opus": "ghost-model" } });
        let known = vec![crate::application::models_listing::ModelListingItem {
            id: "known-model".to_string(),
            object: "model".to_string(),
            created: 0,
            owned_by: "silk".to_string(),
            model_mapping_id: None,
        }];
        let missing = missing_referenced_models("claude_code", &cfg, &known);
        assert_eq!(missing, vec!["ghost-model"]);
    }
}