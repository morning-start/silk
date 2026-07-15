use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::{require_db, require_found, validate_non_empty, ServiceError};
use crate::models::NewProfile;
use crate::persistence::common_config_snippet_repo::CommonConfigSnippetRepo;
use crate::persistence::ProfileRepo;

// ---------------------------------------------------------------------------
// AgentConfigWriter trait — 各 Agent 配置写入器接口
// ---------------------------------------------------------------------------

#[async_trait]
pub trait AgentConfigWriter: Send + Sync {
    fn agent_type(&self) -> &'static str;

    /// 读取当前 live 配置
    async fn read_live(&self, home: &Path) -> Result<Option<Vec<u8>>, String>;

    /// 将 Profile 的 config_json 写入 live 配置
    async fn write_live(
        &self,
        home: &Path,
        profile_config: &serde_json::Value,
    ) -> Result<(), String>;

    /// 从 live 配置中移除该 Profile 的影响
    async fn remove_live(&self, home: &Path, _profile_id: &str) -> Result<(), String> {
        // 默认实现：重写 live 配置为空
        self.write_live(home, &serde_json::json!({})).await
    }

    /// 检查当前 live 配置是否被 Silk 管理
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
// JsonConfigWriter — Claude Code / Gemini CLI（单一 current 模式，JSON）
// ---------------------------------------------------------------------------

pub struct JsonConfigWriter {
    relative_path: String,
}

impl JsonConfigWriter {
    pub fn new(relative_path: &str) -> Self {
        Self {
            relative_path: relative_path.to_string(),
        }
    }

    fn live_path(&self, home: &Path) -> PathBuf {
        home.join(&self.relative_path)
    }

    async fn read_json(&self, path: &Path) -> Result<serde_json::Value, String> {
        if !path.exists() {
            return Ok(serde_json::json!({}));
        }
        let data = tokio::fs::read_to_string(path).await.map_err(|e| e.to_string())?;
        serde_json::from_str(&data).map_err(|e| e.to_string())
    }
}

#[async_trait]
impl AgentConfigWriter for JsonConfigWriter {
    fn agent_type(&self) -> &'static str {
        "claude_code"
    }

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
        let mut live = self.read_json(&path).await?;

        // 合并顶层字段
        if let Some(obj) = profile_config.as_object() {
            for (key, value) in obj {
                live[key] = value.clone();
            }
        }

        // 移除内部字段
        live.as_object_mut().map(|obj| {
            obj.remove("_silk_managed");
            obj.remove("_silk_profile_id");
        });

        // 添加管理标记
        live["_silk_managed"] = serde_json::json!(true);

        // 原子写入
        let data = serde_json::to_vec_pretty(&live).map_err(|e| e.to_string())?;
        atomic_write(&path, &data).await
    }
}

// ---------------------------------------------------------------------------
// 原子写入
// ---------------------------------------------------------------------------

async fn atomic_write(path: &Path, data: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("无法获取父目录")?;
    tokio::fs::create_dir_all(parent).await.map_err(|e| e.to_string())?;

    let tmp = parent.join(format!(
        "{}.tmp.{}",
        path.file_name().unwrap().to_str().unwrap(),
        std::process::id()
    ));

    tokio::fs::write(&tmp, data).await.map_err(|e| e.to_string())?;

    // Windows: 先删除目标
    #[cfg(windows)]
    if path.exists() {
        let _ = tokio::fs::remove_file(path).await;
    }

    tokio::fs::rename(&tmp, path).await.map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 用户家目录辅助
// ---------------------------------------------------------------------------

fn user_home_dir() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
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
// Writer 注册表
// ---------------------------------------------------------------------------

fn builtin_writers() -> Vec<Box<dyn AgentConfigWriter>> {
    vec![
        Box::new(JsonConfigWriter::new(".claude/settings.json")),
        // Phase 2: 添加 TomlConfigWriter, AdditiveJsonConfigWriter, YamlConfigWriter
    ]
}

fn writer_for(agent_type: &str) -> Option<Box<dyn AgentConfigWriter>> {
    builtin_writers().into_iter().find(|w| w.agent_type() == agent_type)
}

// ---------------------------------------------------------------------------
// 校验
// ---------------------------------------------------------------------------

fn validate_profile_payload(agent_type: &str, config_json: &str) -> Result<(), ServiceError> {
    validate_non_empty("agent_type", agent_type)?;
    validate_non_empty("config_json", config_json)?;

    // 校验 config_json 为合法 JSON
    serde_json::from_str::<serde_json::Value>(config_json).map_err(|e| {
        ServiceError::BadRequest {
            message: format!("config_json 不是合法 JSON: {}", e),
            code: None,
        }
    })?;

    // 校验 agent_type
    const VALID_AGENTS: &[&str] = &[
        "claude_code", "claude_desktop", "codex",
        "gemini_cli", "opencode", "openclaw", "hermes",
    ];
    if !VALID_AGENTS.contains(&agent_type) {
        return Err(ServiceError::BadRequest {
            message: format!("不支持的 agent_type: {}", agent_type),
            code: None,
        });
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// CRUD
// ---------------------------------------------------------------------------

/// 按 agent_type 列出 Profile
pub async fn list(agent_type: String) -> Result<Vec<ProfileResponse>, ServiceError> {
    let pool = require_db()?;
    let profiles = ProfileRepo::find_by_agent_type(pool, &agent_type).await?;
    Ok(profiles.into_iter().map(ProfileResponse::from).collect())
}

/// 获取单个 Profile
pub async fn get(profile_id: String) -> Result<ProfileResponse, ServiceError> {
    let pool = require_db()?;
    let profile = require_found(
        ProfileRepo::find_by_id(pool, &profile_id).await?,
        "Profile",
    )?;
    Ok(ProfileResponse::from(profile))
}

/// 创建 Profile
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

/// 更新 Profile
pub async fn update(
    profile_id: String,
    payload: UpdateProfilePayload,
) -> Result<ProfileResponse, ServiceError> {
    let pool = require_db()?;

    // 校验存在性
    let _existing = require_found(
        ProfileRepo::find_by_id(pool, &profile_id).await?,
        "Profile",
    )?;

    // 如果提供了 config_json，校验合法性
    if let Some(ref config_json) = payload.config_json {
        serde_json::from_str::<serde_json::Value>(config_json).map_err(|e| {
            ServiceError::BadRequest {
                message: format!("config_json 不是合法 JSON: {}", e),
                code: None,
            }
        })?;
    }

    let update = crate::models::UpdateProfile {
        name: payload.name.map(|n| n.trim().to_string()),
        config_json: payload.config_json,
        is_active: None, // is_active 只能通过 switch 修改
        sort_index: payload.sort_index,
    };

    let profile = require_found(
        ProfileRepo::update(pool, &profile_id, &update).await?,
        "Profile",
    )?;
    Ok(ProfileResponse::from(profile))
}

/// 删除 Profile
pub async fn delete(profile_id: String) -> Result<bool, ServiceError> {
    let pool = require_db()?;
    ProfileRepo::delete(pool, &profile_id).await.map_err(ServiceError::from)
}

// ---------------------------------------------------------------------------
// 切换（核心）
// ---------------------------------------------------------------------------

/// 切换 Profile：激活指定 Profile，写入 live 配置
pub async fn switch(
    agent_type: String,
    profile_id: String,
) -> Result<SwitchResult, ServiceError> {
    let pool = require_db()?;

    // 1. 查找 Profile
    let profile = require_found(
        ProfileRepo::find_by_id(pool, &profile_id).await?,
        "Profile",
    )?;

    // 2. 校验 agent_type 匹配
    if profile.agent_type != agent_type {
        return Err(ServiceError::BadRequest {
            message: format!("Profile 的 agent_type ({}) 与请求 ({}) 不匹配", profile.agent_type, agent_type),
            code: None,
        });
    }

    let mut warnings = Vec::new();

    // 3. 获取 Writer
    let writer = match writer_for(&agent_type) {
        Some(w) => w,
        None => {
            // 对于 Phase 1 不支持的 agent_type，仅更新 DB 状态，不写文件
            return switch_db_only(pool, &agent_type, &profile_id).await;
        }
    };

    // 4. 解析配置
    let config: serde_json::Value = serde_json::from_str(&profile.config_json).map_err(|e| {
        ServiceError::BadRequest {
            message: format!("config_json 解析失败: {}", e),
            code: None,
        }
    })?;

    // 5. 读取通用配置片段（如果有）
    let _snippet = CommonConfigSnippetRepo::find_by_agent(pool, &agent_type).await?;

    // 构建有效配置
    let effective_config = build_effective_config(&config, _snippet.as_ref());

    // 6. 写入 live 配置
    let home = user_home_dir();
    if let Err(e) = writer.write_live(&home, &effective_config).await {
        warnings.push(format!("写入 live 配置失败: {}", e));
    }

    // 7. 更新 DB 状态
    ProfileRepo::deactivate_all(pool, &agent_type).await?;
    ProfileRepo::activate(pool, &profile_id).await?;

    // 8. 判断是否需要重启
    let requires_restart = match agent_type.as_str() {
        "opencode" | "openclaw" => false,
        _ => true,
    };

    if requires_restart {
        warnings.push("请重启终端/应用以使配置生效".to_string());
    }

    Ok(SwitchResult {
        success: true,
        warnings,
        requires_restart,
    })
}

/// 仅更新 DB 状态（无 Writer 的 Agent 类型）
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

/// 构建有效配置（合并通用配置片段）
fn build_effective_config(
    config: &serde_json::Value,
    snippet: Option<&crate::persistence::CommonConfigSnippet>,
) -> serde_json::Value {
    let mut effective = config.clone();

    if let Some(snippet) = snippet {
        if let Ok(snippet_val) = serde_json::from_str::<serde_json::Value>(&snippet.content) {
            // 深度合并
            json_deep_merge(&mut effective, &snippet_val);
        }
    }

    // 添加管理标记
    if let Some(obj) = effective.as_object_mut() {
        obj.insert("_silk_managed".to_string(), serde_json::json!(true));
    }

    effective
}

/// JSON 深度合并：将 source 合并到 target
fn json_deep_merge(target: &mut serde_json::Value, source: &serde_json::Value) {
    if let (Some(t), Some(s)) = (target.as_object_mut(), source.as_object()) {
        for (key, value) in s {
            if let Some(existing) = t.get(key) {
                if existing.is_object() && value.is_object() {
                    json_deep_merge(&mut t[key], value);
                    continue;
                }
            }
            t.insert(key.clone(), value.clone());
        }
    }
}