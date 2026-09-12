//! 内置模型目录（模型池元数据补充）
//!
//! 元数据以独立文件 `model-catalog.json` 存储（应用数据目录），不嵌入代码，
//! 用户可直接编辑扩展。仅用于**模型池（model_mappings）**的元数据补充，
//! 渠道（Provider）模型不受影响。合并规则：DB 字段有值优先，空值从目录补。

use std::collections::HashMap;
use std::path::Path;
use std::sync::{OnceLock, RwLock};

use serde::Deserialize;

/// 目录文件 schema 版本
const SCHEMA_VERSION: i64 = 1;

/// 模型元数据条目（与 model-catalog.json 的 models[] 元素一一对应）
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ModelCatalogEntry {
    /// 主键：与模型池 `model_mappings.model_name` 精确匹配
    pub model_name: String,
    pub max_input_tokens: Option<i64>,
    pub max_context_tokens: Option<i64>,
    pub max_output_tokens: Option<i64>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub vendor: String,
    pub knowledge_cutoff: Option<String>,
    #[serde(default)]
    pub model_family: String,
    pub reference_url: Option<String>,
    pub reasoning: Option<bool>,
    #[serde(default)]
    pub input_types: Vec<String>,
}

/// 目录文件根结构
#[derive(Debug, Default, Deserialize)]
pub struct ModelCatalogFile {
    #[serde(default)]
    pub version: i64,
    #[serde(default)]
    pub models: Vec<ModelCatalogEntry>,
}

/// 加载后的目录（按 model_name 索引）
#[derive(Debug, Default)]
pub struct ModelCatalog {
    by_name: HashMap<String, ModelCatalogEntry>,
}

impl ModelCatalog {
    /// 从 JSON 文件加载目录。文件缺失返回空目录（不报错），解析失败返回 Err。
    pub fn load(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path).map_err(|e| format!("读取目录失败: {e}"))?;
        let file: ModelCatalogFile =
            serde_json::from_str(&text).map_err(|e| format!("解析目录失败: {e}"))?;
        if file.version != SCHEMA_VERSION {
            tracing::warn!(
                version = file.version,
                "模型目录版本不匹配，期望 v{SCHEMA_VERSION}，仍按当前 schema 解析"
            );
        }
        Ok(Self::load_from_file(file))
    }

    /// 从已解析的目录文件构建索引（纯函数，供 load 与测试复用）
    pub fn load_from_file(file: ModelCatalogFile) -> Self {
        let by_name = file
            .models
            .into_iter()
            .map(|m| (m.model_name.clone(), m))
            .collect();
        Self { by_name }
    }

    /// 按模型名查询元数据
    pub fn get(&self, model_name: &str) -> Option<&ModelCatalogEntry> {
        self.by_name.get(model_name)
    }

    /// 条目数量（供日志/测试）
    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }
}

/// 全局目录缓存（进程级单例；watcher 可重载）
static CATALOG: OnceLock<RwLock<ModelCatalog>> = OnceLock::new();

/// 初始化全局目录缓存（启动时调用）。文件缺失/解析失败均不阻断启动。
pub fn init(path: &Path) {
    match ModelCatalog::load(path) {
        Ok(catalog) => {
            tracing::info!("模型目录已加载: {} 条", catalog.len());
            let _ = CATALOG.set(RwLock::new(catalog));
        }
        Err(e) => {
            tracing::warn!(error = %e, "模型目录加载失败，使用空目录");
            let _ = CATALOG.set(RwLock::new(ModelCatalog::default()));
        }
    }
}

/// 重载全局目录缓存（外部文件变更时由 watcher 调用）。失败时保留旧缓存。
pub fn reload(path: &Path) {
    match ModelCatalog::load(path) {
        Ok(catalog) => {
            if let Some(c) = CATALOG.get() {
                if let Ok(mut guard) = c.write() {
                    *guard = catalog;
                    tracing::info!("模型目录已重载");
                }
            }
        }
        Err(e) => tracing::warn!(error = %e, "模型目录重载失败，保留旧缓存"),
    }
}

/// 查询模型元数据（返回克隆，避免调用方持有锁）。
/// 目录未初始化或未命中时返回 None。
pub fn get_entry(model_name: &str) -> Option<ModelCatalogEntry> {
    let catalog = CATALOG.get()?;
    let guard = catalog.read().ok()?;
    guard.get(model_name).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_temp(name: &str, content: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("model-catalog-{name}-{}.json", std::process::id()));
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn load_missing_file_returns_empty() {
        let catalog =
            ModelCatalog::load(Path::new("/nonexistent/model-catalog.json")).expect("不应报错");
        assert!(catalog.is_empty());
    }

    #[test]
    fn load_valid_file_indexes_by_name() {
        let path = write_temp(
            "valid",
            r#"{
                "version": 1,
                "models": [
                    {
                        "model_name": "gpt-4o",
                        "max_context_tokens": 128000,
                        "capabilities": ["chat", "vision"],
                        "reasoning": false,
                        "input_types": ["text", "image"]
                    }
                ]
            }"#,
        );
        let catalog = ModelCatalog::load(&path).expect("解析成功");
        assert_eq!(catalog.len(), 1);
        let entry = catalog.get("gpt-4o").expect("按名命中");
        assert_eq!(entry.max_context_tokens, Some(128000));
        assert_eq!(entry.capabilities, vec!["chat", "vision"]);
        assert_eq!(entry.reasoning, Some(false));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn load_invalid_json_returns_err() {
        let path = write_temp("invalid", "{ not valid json ");
        assert!(ModelCatalog::load(&path).is_err());
        std::fs::remove_file(&path).ok();
    }
}
