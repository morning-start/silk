//! 配置写入基础设施（重建自 cc-switch 复刻需要）
//!
//! ConfigFormat 格式枚举、LiveSnapshot 写前快照、原子写与通用合并工具。

use std::path::Path;

/// 配置格式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    Json,
    Toml,
    Yaml,
}

/// 写前快照：备份 live 文件内容，写入失败时 restore 恢复原状。
/// 对齐 cc-switch 的 with_rollback / snapshot_files 语义。
pub struct LiveSnapshot {
    path: std::path::PathBuf,
    backup: Option<Vec<u8>>,
}

impl LiveSnapshot {
    /// 读取并备份文件（不存在时备份为 None，restore 时删除新写入的文件）
    pub fn take(path: &Path) -> Result<Self, String> {
        let backup = if path.exists() {
            Some(std::fs::read(path).map_err(|e| format!("备份读取失败: {e}"))?)
        } else {
            None
        };
        Ok(Self {
            path: path.to_path_buf(),
            backup,
        })
    }

    /// 恢复快照（写入失败回滚）
    pub fn restore(&self) -> Result<(), String> {
        match &self.backup {
            Some(bytes) => {
                if let Some(parent) = self.path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                std::fs::write(&self.path, bytes).map_err(|e| format!("恢复快照失败: {e}"))
            }
            None => {
                let _ = std::fs::remove_file(&self.path);
                Ok(())
            }
        }
    }
}

/// 读取 JSON 值（异步；文件不存在返回 None）
pub async fn read_to_value_async(path: &Path) -> Result<Option<serde_json::Value>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let data = tokio::fs::read(path).await.map_err(|e| format!("读取失败: {e}"))?;
    let text = String::from_utf8(data).map_err(|e| format!("非 UTF-8: {e}"))?;
    let v = serde_json::from_str(&text).map_err(|e| format!("JSON 解析失败: {e}"))?;
    Ok(Some(v))
}

/// 原子写 JSON（先写临时文件再重命名，避免半写状态）
pub fn write_to_path_atomic(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|e| format!("序列化失败: {e}"))?;
    write_text_atomic(path, &text)
}

/// 原子写文本
pub fn write_text_atomic(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("写入临时文件失败: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("重命名失败: {e}"))?;
    // 记录本次自身写入（供外部文件 watcher 区分「应用写入」与「外部编辑」，
    // 避免 gateway.json 保存触发监听循环重启）
    record_self_write(path, text);
    Ok(())
}

/// 最近一次由应用自身原子写入的文件内容（路径 → 内容摘要）
use std::sync::Mutex;

static LAST_SELF_WRITES: Mutex<Vec<(std::path::PathBuf, String)>> = Mutex::new(Vec::new());

fn record_self_write(path: &Path, text: &str) {
    if let Ok(mut guard) = LAST_SELF_WRITES.lock() {
        guard.retain(|(p, _)| p != path);
        guard.push((path.to_path_buf(), text.to_string()));
        // 只保留最近 8 个，防止无界增长
        if guard.len() > 8 {
            guard.remove(0);
        }
    }
}

/// 判断该文件当前内容是否与应用最近一次自身写入一致（即未发生外部编辑）
pub fn is_self_write(path: &Path, content: &str) -> bool {
    if let Ok(guard) = LAST_SELF_WRITES.lock() {
        guard
            .iter()
            .any(|(p, text)| p == path && text == content)
    } else {
        false
    }
}

/// 深合并：target 中不存在的键才从 source 补充（不覆盖已有键）
pub fn json_deep_merge(target: &mut serde_json::Value, source: &serde_json::Value) {
    match (target, source) {
        (serde_json::Value::Object(t), serde_json::Value::Object(s)) => {
            for (k, v) in s {
                match t.get_mut(k) {
                    Some(existing) => json_deep_merge(existing, v),
                    None => {
                        t.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (t, s) => {
            if t.is_null() {
                *t = s.clone();
            }
        }
    }
}

/// 校验配置文本格式正确
pub fn validate_config_text(text: &str, fmt: ConfigFormat) -> Result<(), String> {
    match fmt {
        ConfigFormat::Json => serde_json::from_str::<serde_json::Value>(text)
            .map(|_| ())
            .map_err(|e| format!("JSON 格式错误: {e}")),
        ConfigFormat::Toml => toml::from_str::<toml::Value>(text)
            .map(|_| ())
            .map_err(|e| format!("TOML 格式错误: {e}")),
        ConfigFormat::Yaml => serde_yaml::from_str::<serde_json::Value>(text)
            .map(|_| ())
            .map_err(|e| format!("YAML 格式错误: {e}")),
    }
}
