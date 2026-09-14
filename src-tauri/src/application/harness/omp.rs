use std::path::{Path, PathBuf};
use std::process::Command;

use crate::application::config_writer::ConfigFormat;
use async_trait::async_trait;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// OMP：写入 models.yml 的 `providers.<id>` 段（对齐 omp-switch 的 provider 映射结构）。
///
/// models.yml 结构（OMP 原生）：
/// ```yaml
/// providers:
///   <id>:
///     name: ...
///     baseUrl: ...
///     apiKey: ...
///     api: ...
///     headers: {...}
///     models:
///       - id: ...
///         name: ...
///         api: ...
///         reasoning: ...
///         input: [text, image]
///         cost: {input, output, cacheRead, cacheWrite}
///         contextWindow: ...
///         maxTokens: ...
/// ```
/// 激活/停用语义与 opencode 一致：累加模式，一个预设 = providers.<id> 一段。
pub struct OmpWriter;

/// 解析 OMP agent 配置目录（对齐 omp-switch get_omp_agent_dir）：
/// 1. `PI_CODING_AGENT_DIR` 环境变量；2. `omp config path` 命令；3. `~/.omp/agent` 兜底
fn omp_agent_dir(home: &Path) -> PathBuf {
    if let Ok(dir) = std::env::var("PI_CODING_AGENT_DIR") {
        let dir = dir.trim();
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    for executable in omp_executable_candidates() {
        let mut command = Command::new(executable);
        command.args(["config", "path"]);
        #[cfg(windows)]
        command.creation_flags(CREATE_NO_WINDOW);
        if let Ok(output) = command.output() {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    return PathBuf::from(path);
                }
            }
        }
    }
    home.join(".omp").join("agent")
}

#[cfg(windows)]
fn omp_executable_candidates() -> [&'static str; 2] {
    ["omp", "omp.cmd"]
}

#[cfg(not(windows))]
fn omp_executable_candidates() -> [&'static str; 1] {
    ["omp"]
}

/// 从 settings 解析 provider id（对齐 opencode：_silk_provider_id → id → name → silk）
fn provider_id(settings: &serde_json::Value) -> String {
    settings
        .get("_silk_provider_id")
        .or_else(|| settings.get("id"))
        .or_else(|| settings.get("name"))
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("silk")
        .to_string()
}

#[async_trait]
impl super::HarnessWriter for OmpWriter {
    fn agent_type(&self) -> &'static str {
        "omp"
    }

    fn live_path(&self, home: &Path) -> PathBuf {
        omp_agent_dir(home).join("models.yml")
    }

    fn config_format(&self) -> ConfigFormat {
        ConfigFormat::Yaml
    }

    async fn write_live_inner(
        &self,
        home: &Path,
        settings: &serde_json::Value,
        _remove_keys: &[String],
    ) -> Result<(), String> {
        let path = self.live_path(home);
        let mut live: serde_json::Value = if path.exists() {
            let data = tokio::fs::read(&path)
                .await
                .map_err(|e| format!("读取 models.yml 失败: {e}"))?;
            let text = String::from_utf8(data).map_err(|e| format!("非 UTF-8: {e}"))?;
            if text.trim().is_empty() {
                serde_json::json!({})
            } else {
                serde_yaml::from_str(&text).map_err(|e| format!("YAML 解析失败: {e}"))?
            }
        } else {
            serde_json::json!({})
        };

        let root = live
            .as_object_mut()
            .ok_or_else(|| "models.yml 根不是对象".to_string())?;
        let providers = root
            .entry("providers".to_string())
            .or_insert_with(|| serde_json::json!({}));
        let providers = providers
            .as_object_mut()
            .ok_or_else(|| "providers 不是对象".to_string())?;

        let id = provider_id(settings);
        let mut entry = settings.clone();
        if let Some(object) = entry.as_object_mut() {
            object.remove("_silk_provider_id");
            object.insert("_silk_managed".to_string(), serde_json::json!(true));
        }
        providers.insert(id, entry);

        let text =
            serde_yaml::to_string(&live).map_err(|e| format!("YAML 序列化失败: {e}"))?;
        crate::application::config_writer::write_text_atomic(&path, &text)
            .map_err(|e| format!("写入 models.yml 失败: {e}"))
    }

    /// 取消激活：从 models.yml 删除本 preset 对应的 providers.<id> 段（快照回滚）。
    /// OMP 为累加模式，停用一个 provider 不影响其他已激活 provider。
    async fn remove_from_live(
        &self,
        home: &Path,
        settings: &serde_json::Value,
    ) -> Result<(), String> {
        let path = self.live_path(home);
        let snapshot = crate::application::config_writer::LiveSnapshot::take(&path)
            .map_err(|e| format!("备份失败: {e}"))?;
        let result: Result<(), String> = async {
            let mut live: serde_json::Value = if path.exists() {
                let data = tokio::fs::read(&path)
                    .await
                    .map_err(|e| format!("读取 models.yml 失败: {e}"))?;
                let text = String::from_utf8(data).map_err(|e| format!("非 UTF-8: {e}"))?;
                if text.trim().is_empty() {
                    serde_json::json!({})
                } else {
                    serde_yaml::from_str(&text).map_err(|e| format!("YAML 解析失败: {e}"))?
                }
            } else {
                return Ok(()); // 无 live 文件，无需移除
            };
            let id = provider_id(settings);
            let removed = match live
                .as_object_mut()
                .ok_or_else(|| "models.yml 根不是对象".to_string())?
                .get_mut("providers")
                .and_then(|value| value.as_object_mut())
            {
                Some(providers) => providers.remove(&id).is_some(),
                None => false,
            };
            if removed {
                let text =
                    serde_yaml::to_string(&live).map_err(|e| format!("YAML 序列化失败: {e}"))?;
                crate::application::config_writer::write_text_atomic(&path, &text)
                    .map_err(|e| format!("写入 models.yml 失败: {e}"))?;
            }
            Ok(())
        }
        .await;
        if let Err(error) = &result {
            let _ = snapshot.restore();
            return Err(format!("移除失败，已恢复快照: {error}"));
        }
        result
    }

    fn extract_startup_settings(&self, live: &serde_json::Value) -> Vec<serde_json::Value> {
        let Some(providers) = live
            .get("providers")
            .and_then(|value| value.as_object())
        else {
            return Vec::new();
        };
        providers
            .iter()
            .filter_map(|(id, value)| {
                let mut entry = value.as_object()?.clone();
                entry.remove("_silk_managed");
                entry.insert("id".to_string(), serde_json::Value::String(id.clone()));
                Some(serde_json::Value::Object(entry))
            })
            .collect()
    }

    fn extract_settings(
        &self,
        live: &serde_json::Value,
        stored: &serde_json::Value,
    ) -> Option<serde_json::Value> {
        let providers = live.get("providers")?.as_object()?;
        let id = stored
            .get("_silk_provider_id")
            .or_else(|| stored.get("id"))
            .or_else(|| stored.get("name"))
            .and_then(|value| value.as_str())?;
        let mut entry = providers.get(id).cloned()?;
        if let Some(object) = entry.as_object_mut() {
            object.remove("_silk_managed");
            object.insert("id".to_string(), serde_json::Value::String(id.to_string()));
        }
        Some(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::harness::HarnessWriter;

    /// 串行化依赖 `PI_CODING_AGENT_DIR` 环境变量的写测试：
    /// 本机若装有 omp，`omp config path` 会命中真实配置目录，必须用 env 隔离。
    /// 共享 harness 模块的 ENV_LOCK（与 preset_service 测试共用，避免并行 env 竞争）。
    use crate::application::harness::ENV_LOCK;

    /// 构造临时 home 目录 + 预置 models.yml（含一个未管理 provider），
    /// 并设置 `PI_CODING_AGENT_DIR` 指向临时 agent 目录（避免命中真实 omp 配置）。
    /// 调用方必须持有 `ENV_LOCK` 串行执行。
    fn temp_home_with_models_yaml() -> (std::path::PathBuf, std::path::PathBuf) {
        let base = std::env::temp_dir().join(format!("omp-writer-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let agent_dir = base.join(".omp").join("agent");
        std::fs::create_dir_all(&agent_dir).expect("mkdir");
        // 优先走 env，屏蔽 `omp config path` 探测
        std::env::set_var("PI_CODING_AGENT_DIR", &agent_dir);
        let yaml = r#"providers:
  manual:
    name: Manual
    baseUrl: https://manual.example/v1
"#;
        std::fs::write(agent_dir.join("models.yml"), yaml).expect("write yaml");
        (base, agent_dir)
    }

    #[tokio::test]
    async fn write_merges_provider_section_without_touching_others() {
        let _guard = ENV_LOCK.lock().unwrap();
        let (base, agent_dir) = temp_home_with_models_yaml();
        let home = base.as_path();
        let writer = OmpWriter;
        let settings = serde_json::json!({
            "_silk_provider_id": "silk-test",
            "name": "Silk Test",
            "baseUrl": "http://127.0.0.1:1877/v1",
            "apiKey": "sk-silk-test",
            "api": "openai-completions",
            "models": [
                {"id": "gpt-4o", "name": "GPT-4o", "reasoning": false, "input": ["text", "image"],
                 "contextWindow": 128000, "maxTokens": 16384,
                 "cost": {"input": 2.5, "output": 10.0}}
            ]
        });
        writer.write_live(home, &settings, &[]).await.expect("write");

        let content = std::fs::read_to_string(agent_dir.join("models.yml")).expect("read");
        // ① 未管理 provider 保留
        assert!(content.contains("manual"), "未管理 provider 不应被覆盖");
        // ② 新 provider 段写入
        assert!(content.contains("silk-test"), "新 provider 段应写入");
        assert!(content.contains("_silk_managed: true"), "应带接管标记");
        // ③ 结构化模型元数据保留
        assert!(content.contains("contextWindow"), "模型 contextWindow 应保留");
        // ④ 未声明的 cacheRead 不应凭空出现
        assert!(!content.contains("cacheRead"), "未声明字段不应出现");
    }

    #[tokio::test]
    async fn remove_from_live_strips_only_own_provider() {
        let _guard = ENV_LOCK.lock().unwrap();
        let (base, agent_dir) = temp_home_with_models_yaml();
        let home = base.as_path();
        let writer = OmpWriter;

        // 先写入两个 provider（模拟多激活）
        let settings_a = serde_json::json!({"_silk_provider_id": "silk-a", "name": "A"});
        let settings_b = serde_json::json!({"_silk_provider_id": "silk-b", "name": "B"});
        writer.write_live(home, &settings_a, &[]).await.expect("write a");
        writer.write_live(home, &settings_b, &[]).await.expect("write b");

        // 停用 silk-a
        writer.remove_from_live(home, &settings_a).await.expect("remove a");

        let content = std::fs::read_to_string(agent_dir.join("models.yml")).expect("read");
        assert!(!content.contains("silk-a"), "silk-a 应被剥离");
        assert!(content.contains("silk-b"), "silk-b 应保留");
        assert!(content.contains("manual"), "未管理 provider 应保留");
    }

    #[test]
    fn extract_startup_imports_all_provider_entries() {
        let live = serde_json::json!({
            "providers": {
                "relay": {"name": "Relay", "_silk_managed": true, "baseUrl": "https://relay.example"},
                "manual": {"name": "Manual", "baseUrl": "https://manual.example"}
            }
        });
        let writer = OmpWriter;
        let imported = writer.extract_startup_settings(&live);
        assert_eq!(imported.len(), 2);
        let relay = imported.iter().find(|item| item["id"] == "relay").expect("relay");
        assert!(relay.get("_silk_managed").is_none(), "接管标记应剥离");
        assert!(imported.iter().any(|item| item["id"] == "manual"));
    }

    #[test]
    fn extract_settings_selects_stored_provider() {
        let live = serde_json::json!({
            "providers": {
                "silk-test": {"name": "Silk Test", "_silk_managed": true, "baseUrl": "http://127.0.0.1:1877/v1"}
            }
        });
        let stored = serde_json::json!({"_silk_provider_id": "silk-test"});
        let writer = OmpWriter;
        let extracted = writer.extract_settings(&live, &stored).expect("provider");
        assert_eq!(extracted["baseUrl"], "http://127.0.0.1:1877/v1");
        assert!(extracted.get("_silk_managed").is_none());
    }
}
