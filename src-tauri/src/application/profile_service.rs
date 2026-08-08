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
        if self.merge_strategy() != MergeStrategy::IntoProvider {
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
        config_writer::remove_provider_from_live_async(&path, profile_id).await
    }

    async fn is_managed(&self, home: &Path) -> Result<bool, String> {
        let path = self.live_path(home);
        if !path.exists() {
            return Ok(false);
        }
        let val = config_writer::read_to_value_async(&path).await?;
        // 检查是否有任何 provider 被 silk 管理
        if let Some(providers) = val.get("providers").and_then(|v| v.as_object()) {
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
        config_writer::remove_provider_from_live_async(&path, profile_id).await
    }

    async fn is_managed(&self, home: &Path) -> Result<bool, String> {
        let path = self.live_path(home);
        if !path.exists() {
            return Ok(false);
        }
        let val = config_writer::read_to_value_async(&path).await?;
        if let Some(providers) = val.get("providers").and_then(|v| v.as_object()) {
            for (_id, provider) in providers {
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

    // 注入 base_url + api_key
    if let Ok(settings) = crate::models::GatewaySettings::load(
        crate::get_settings_path().ok_or_else(|| ServiceError::Internal {
            message: "无法获取设置路径".to_string(),
            detail: None,
        })?,
    ) {
        let base_url = format!("http://{}:{}/v1", settings.bind_host, settings.bind_port);
        if let Some(obj) = effective_config.as_object_mut() {
            obj.insert("base_url".to_string(), serde_json::json!(base_url));
            obj.insert("api_key".to_string(), serde_json::json!(crate::application::gateway_key_service::builtin_key_value()));
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