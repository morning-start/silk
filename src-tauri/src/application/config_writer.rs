use std::path::{Path, PathBuf};

// ============================================================================
// ConfigFormat
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    Json,
    Toml,
    Yaml,
}

impl ConfigFormat {
    pub fn from_path(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()? {
            "json" => Some(Self::Json),
            "toml" => Some(Self::Toml),
            "yaml" | "yml" => Some(Self::Yaml),
            _ => None,
        }
    }
}

// ============================================================================
// 读取/写入（内部统一用 serde_json::Value）
// ============================================================================

pub fn read_to_value(path: &Path) -> Result<serde_json::Value, String> {
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    if text.trim().is_empty() {
        return Ok(serde_json::json!({}));
    }
    let fmt = ConfigFormat::from_path(path)
        .ok_or_else(|| format!("不支持的文件格式: {}", path.display()))?;
    match fmt {
        ConfigFormat::Json => serde_json::from_str(&text).map_err(|e| e.to_string()),
        ConfigFormat::Toml => {
            let v: serde_json::Value = toml::from_str(&text).map_err(|e| e.to_string())?;
            Ok(v)
        }
        ConfigFormat::Yaml => {
            let v: serde_json::Value = serde_yaml::from_str(&text).map_err(|e| e.to_string())?;
            Ok(v)
        }
    }
}

pub fn write_from_value(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    let fmt = ConfigFormat::from_path(path)
        .ok_or_else(|| format!("不支持的文件格式: {}", path.display()))?;
    let text = match fmt {
        ConfigFormat::Json => serde_json::to_string_pretty(value).map_err(|e| e.to_string())?,
        ConfigFormat::Toml => toml::to_string_pretty(value).map_err(|e| e.to_string())?,
        ConfigFormat::Yaml => serde_yaml::to_string(value).map_err(|e| e.to_string())?,
    };
    atomic_write(path, text.as_bytes())
}

pub async fn read_to_value_async(path: &Path) -> Result<serde_json::Value, String> {
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }
    let text = tokio::fs::read_to_string(path).await.map_err(|e| e.to_string())?;
    if text.trim().is_empty() {
        return Ok(serde_json::json!({}));
    }
    let fmt = ConfigFormat::from_path(path)
        .ok_or_else(|| format!("不支持的文件格式: {}", path.display()))?;
    match fmt {
        ConfigFormat::Json => serde_json::from_str(&text).map_err(|e| e.to_string()),
        ConfigFormat::Toml => {
            let v: serde_json::Value = toml::from_str(&text).map_err(|e| e.to_string())?;
            Ok(v)
        }
        ConfigFormat::Yaml => {
            let v: serde_json::Value = serde_yaml::from_str(&text).map_err(|e| e.to_string())?;
            Ok(v)
        }
    }
}

pub async fn write_from_value_async(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    let fmt = ConfigFormat::from_path(path)
        .ok_or_else(|| format!("不支持的文件格式: {}", path.display()))?;
    let text = match fmt {
        ConfigFormat::Json => serde_json::to_string_pretty(value).map_err(|e| e.to_string())?,
        ConfigFormat::Toml => toml::to_string_pretty(value).map_err(|e| e.to_string())?,
        ConfigFormat::Yaml => serde_yaml::to_string(value).map_err(|e| e.to_string())?,
    };
    atomic_write_async(path, text.as_bytes()).await
}

// ============================================================================
// 原子写入
// ============================================================================

pub fn atomic_write(path: &Path, data: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("无法获取父目录")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;

    let tmp = parent.join(format!(
        "{}.tmp.{}",
        path.file_name().unwrap().to_str().unwrap(),
        std::process::id()
    ));

    std::fs::write(&tmp, data).map_err(|e| e.to_string())?;

    #[cfg(windows)]
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }

    std::fs::rename(&tmp, path).map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn atomic_write_async(path: &Path, data: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("无法获取父目录")?;
    tokio::fs::create_dir_all(parent).await.map_err(|e| e.to_string())?;

    let tmp = parent.join(format!(
        "{}.tmp.{}",
        path.file_name().unwrap().to_str().unwrap(),
        std::process::id()
    ));

    tokio::fs::write(&tmp, data).await.map_err(|e| e.to_string())?;

    #[cfg(windows)]
    if path.exists() {
        let _ = tokio::fs::remove_file(path).await;
    }

    tokio::fs::rename(&tmp, path).await.map_err(|e| e.to_string())?;
    Ok(())
}

// ============================================================================
// LiveSnapshot — 写入前备份，失败时恢复
// ============================================================================

pub enum LiveSnapshot {
    Backup { backup_path: PathBuf },
    Noop,
}

impl LiveSnapshot {
    /// 创建备份快照
    pub fn take(live_path: &Path) -> Result<Self, String> {
        if !live_path.exists() {
            return Ok(Self::Noop);
        }
        let content = std::fs::read(live_path).map_err(|e| e.to_string())?;
        let backup_path = live_path.with_extension("silk.bak");
        std::fs::write(&backup_path, &content).map_err(|e| e.to_string())?;
        Ok(Self::Backup { backup_path })
    }

    /// 恢复备份
    pub fn restore(self) -> Result<(), String> {
        match self {
            Self::Backup { backup_path } => {
                let live_path = backup_path.with_extension("");
                if backup_path.exists() {
                    #[cfg(windows)]
                    if live_path.exists() {
                        let _ = std::fs::remove_file(&live_path);
                    }
                    std::fs::rename(&backup_path, &live_path).map_err(|e| e.to_string())?;
                }
                Ok(())
            }
            Self::Noop => Ok(()),
        }
    }
}

// ============================================================================
// JSON 深度合并/移除
// ============================================================================

/// 深度合并：将 source 合并到 target（仅替换/添加 source 中存在的 key）
pub fn json_deep_merge(target: &mut serde_json::Value, source: &serde_json::Value) {
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

/// 深度移除：从 target 中移除与 source 完全匹配的 key
pub fn json_deep_remove(target: &mut serde_json::Value, source: &serde_json::Value) {
    if let (Some(t), Some(s)) = (target.as_object_mut(), source.as_object()) {
        for (key, value) in s {
            if let Some(existing) = t.get(key) {
                if existing.is_object() && value.is_object() {
                    json_deep_remove(&mut t[key], value);
                    if t[key].as_object().map_or(false, |o| o.is_empty()) {
                        t.remove(key);
                    }
                    continue;
                }
                if existing == value {
                    t.remove(key);
                }
            }
        }
    }
}

// ============================================================================
// 合并策略
// ============================================================================

/// 配置写入策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStrategy {
    /// 顶层合并 — 替换/添加 profile_config 中存在的顶层 key
    TopLevel,
    /// 合并到指定子对象（Gemini CLI → env）
    IntoSubObject(&'static str),
    /// 累加模式 — 合并到 providers.{id}（OpenCode / OpenClaw / Hermes）
    IntoProvider,
}

/// 将 profile 配置合并写入 live 配置文件（仅替换对应部分）
pub fn merge_into_live(
    live_path: &Path,
    profile_config: &serde_json::Value,
    strategy: MergeStrategy,
) -> Result<(), String> {
    let _snapshot = LiveSnapshot::take(live_path)?;
    let mut live = read_to_value(live_path)?;

    apply_merge(&mut live, profile_config, strategy);

    if let Err(e) = write_from_value(live_path, &live) {
        let _ = _snapshot.restore();
        return Err(e);
    }
    Ok(())
}

pub async fn merge_into_live_async(
    live_path: &Path,
    profile_config: &serde_json::Value,
    strategy: MergeStrategy,
) -> Result<(), String> {
    let mut live = read_to_value_async(live_path).await?;

    apply_merge(&mut live, profile_config, strategy);

    write_from_value_async(live_path, &live).await
}

fn apply_merge(live: &mut serde_json::Value, profile_config: &serde_json::Value, strategy: MergeStrategy) {
    match strategy {
        MergeStrategy::TopLevel => {
            json_deep_merge(live, profile_config);
            if let Some(obj) = live.as_object_mut() {
                obj.insert("_silk_managed".to_string(), serde_json::json!(true));
            }
        }
        MergeStrategy::IntoSubObject(sub) => {
            if !live.is_object() {
                *live = serde_json::json!({});
            }
            let obj = live.as_object_mut().unwrap();
            if !obj.contains_key(sub) {
                obj.insert(sub.to_string(), serde_json::json!({}));
            }
            json_deep_merge(obj.get_mut(sub).unwrap(), profile_config);
            obj.insert("_silk_managed".to_string(), serde_json::json!(true));
        }
        MergeStrategy::IntoProvider => {
            if !live.is_object() {
                *live = serde_json::json!({});
            }
            let live_obj = live.as_object_mut().unwrap();
            if !live_obj.contains_key("providers") {
                live_obj.insert("providers".to_string(), serde_json::json!({}));
            }

            let provider_id = profile_config
                .get("_silk_provider_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    profile_config
                        .as_object()
                        .and_then(|o| o.keys().next().cloned())
                        .unwrap_or_else(|| "default".to_string())
                });

            let mut config = profile_config.clone();
            config.as_object_mut().map(|o| o.remove("_silk_provider_id"));

            let providers = live_obj.get_mut("providers").unwrap();
            if !providers.is_object() {
                *providers = serde_json::json!({});
            }
            let providers_obj = providers.as_object_mut().unwrap();
            providers_obj.insert(provider_id.clone(), config);

            if let Some(entry) = providers_obj.get_mut(&provider_id) {
                if let Some(e) = entry.as_object_mut() {
                    e.insert("_silk_managed".to_string(), serde_json::json!(true));
                }
            }
        }
    }
}

/// 从 live 配置中移除指定 provider（累加模式）
pub fn remove_provider_from_live(live_path: &Path, provider_id: &str) -> Result<(), String> {
    let mut live = read_to_value(live_path)?;
    if let Some(providers) = live.get_mut("providers") {
        if let Some(obj) = providers.as_object_mut() {
            obj.remove(provider_id);
        }
    }
    write_from_value(live_path, &live)
}

pub async fn remove_provider_from_live_async(live_path: &Path, provider_id: &str) -> Result<(), String> {
    let mut live = read_to_value_async(live_path).await?;
    if let Some(providers) = live.get_mut("providers") {
        if let Some(obj) = providers.as_object_mut() {
            obj.remove(provider_id);
        }
    }
    write_from_value_async(live_path, &live).await
}

// ============================================================================
// 格式验证
// ============================================================================

pub fn validate_config_text(text: &str, format: ConfigFormat) -> Result<(), String> {
    match format {
        ConfigFormat::Json => {
            serde_json::from_str::<serde_json::Value>(text).map_err(|e| format!("JSON 格式错误: {e}"))?;
        }
        ConfigFormat::Toml => {
            let _: serde_json::Value = toml::from_str(text).map_err(|e| format!("TOML 格式错误: {e}"))?;
        }
        ConfigFormat::Yaml => {
            let _: serde_json::Value = serde_yaml::from_str(text).map_err(|e| format!("YAML 格式错误: {e}"))?;
        }
    }
    Ok(())
}