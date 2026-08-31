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

        let cfg = profile_config.as_object().cloned().unwrap_or_default();
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
        config_writer::merge_into_live_async(&path, profile_config, MergeStrategy::IntoSubObject("env")).await
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
        config_writer::merge_into_live_async(&path, profile_config, MergeStrategy::IntoProvider).await
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
        config_writer::merge_into_live_async(&path, profile_config, MergeStrategy::IntoHermesProvider).await
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

    // 注入 base_url + api_key（按 harness 映射到正确字段位置）
    if let Ok(settings) = crate::models::GatewaySettings::load(
        crate::get_settings_path().ok_or_else(|| ServiceError::Internal {
            message: "无法获取设置路径".to_string(),
            detail: None,
        })?,
    ) {
        let base_url = format!("http://{}:{}/v1", settings.bind_host, settings.bind_port);
        let api_key = crate::application::gateway_key_service::builtin_key_value();
        if let Some(obj) = effective_config.as_object_mut() {
            match agent_type.as_str() {
                // Claude Code：写入 env 子对象（settings.json 的 env 作为环境变量注入）
                "claude_code" => {
                    let env = obj
                        .entry("env".to_string())
                        .or_insert_with(|| serde_json::json!({}));
                    if let Some(e) = env.as_object_mut() {
                        e.insert("ANTHROPIC_BASE_URL".to_string(), serde_json::json!(base_url));
                        e.insert("ANTHROPIC_AUTH_TOKEN".to_string(), serde_json::json!(api_key));
                    }
                }
                // Gemini CLI：profile_config 即 env 内容（IntoSubObject("env") 合并），
                // 注入 GOOGLE_GEMINI_BASE_URL / GEMINI_API_KEY
                "gemini_cli" => {
                    obj.insert("GOOGLE_GEMINI_BASE_URL".to_string(), serde_json::json!(base_url));
                    obj.insert("GEMINI_API_KEY".to_string(), serde_json::json!(api_key));
                }
                // Codex：顶层注入，CodexWriter 会搬进 [model_providers.<id>] 表；
                // OpenCode / Hermes：整个 profile_config 即 provider 条目内容，顶层注入即条目内
                _ => {
                    obj.insert("base_url".to_string(), serde_json::json!(base_url));
                    obj.insert("api_key".to_string(), serde_json::json!(api_key));
                }
            }
        }
    }

    let home = crate::get_home_dir().to_path_buf();
    if let Err(e) = writer.write_live(&home, &effective_config).await {
        warnings.push(format!("写入 live 配置失败: {e}"));
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