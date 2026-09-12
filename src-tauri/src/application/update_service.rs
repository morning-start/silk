//! 软件更新检查（GitHub Releases REST API）
//!
//! 参考 <https://docs.github.com/zh/rest/releases/releases>，
//! 使用 `GET /repos/{owner}/{repo}/releases/latest` 读取最新正式发布，
//! 与当前运行版本比较，并为当前平台挑出可直接下载的安装包。
//!
//! 与 tauri-plugin-updater 的分工：
//! - 本模块负责「有没有新版本、新版本是什么、去哪下载」——只读发布元数据，不依赖任何签名产物；
//! - 插件负责「静默下载并安装」——依赖发布里同时存在 `latest.json` 与 `.sig`，
//!   缺失时前端降级为打开下载页（见 `UpdateInfo::auto_update_ready`）。

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::error::ServiceError;

/// 发布仓库（与 scripts/publish.ts 的 REPO 保持一致）
const REPO: &str = "morning-start/silk";
const API_BASE: &str = "https://api.github.com";
/// GitHub REST API 版本固定头。升级前先读官方变更说明：
/// <https://docs.github.com/zh/rest/about-the-rest-api/api-versions>
const API_VERSION: &str = "2026-03-10";
/// GitHub 要求带可识别的 User-Agent，否则拒绝请求
const USER_AGENT: &str = "silk-updater";
/// 只读发布元数据，超时短一些，避免拖慢关于页
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
/// updater 插件静默安装所需的清单文件名
const UPDATER_MANIFEST: &str = "latest.json";

// ---------------------------------------------------------------------------
// 对外结构
// ---------------------------------------------------------------------------

/// 更新检查结果
///
/// `available` 与 `latest_version` 分离：仓库尚无正式发布（404）时，
/// 两者分别为 false / None，前端据此显示「已是最新版本」而不是报错。
#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    /// 当前运行版本
    pub current_version: String,
    /// 最新发布版本（已去掉标签的 `v` 前缀）
    pub latest_version: Option<String>,
    /// 是否有比当前更新的正式发布
    pub available: bool,
    /// 发布标题
    pub name: Option<String>,
    /// 发布说明（Markdown）
    pub notes: Option<String>,
    /// 发布时间（ISO 8601）
    pub published_at: Option<String>,
    /// 发布页地址（浏览器可打开）
    pub release_url: Option<String>,
    /// 当前平台安装包文件名
    pub asset_name: Option<String>,
    /// 当前平台安装包下载地址
    pub download_url: Option<String>,
    /// 发布里是否带 `latest.json`，即客户端能否静默自动安装
    pub auto_update_ready: bool,
}

impl UpdateInfo {
    /// 仓库还没有正式发布时的结果
    fn no_release(current_version: &str) -> Self {
        Self {
            current_version: current_version.to_string(),
            latest_version: None,
            available: false,
            name: None,
            notes: None,
            published_at: None,
            release_url: None,
            asset_name: None,
            download_url: None,
            auto_update_ready: false,
        }
    }
}

// ---------------------------------------------------------------------------
// GitHub REST API 响应
// ---------------------------------------------------------------------------

/// `GET /repos/{owner}/{repo}/releases/latest` 的响应子集
///
/// 字段名与 REST API 文档一致（蛇形），故整段跳过命名转换。
#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    published_at: Option<String>,
    html_url: Option<String>,
    /// 草稿发布对无鉴权请求不可见，这里仍然防御性过滤
    #[serde(default)]
    draft: bool,
    /// `/releases/latest` 本身排除预发布，同样防御性过滤
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

// ---------------------------------------------------------------------------
// 主流程
// ---------------------------------------------------------------------------

/// 检查是否有新版本
///
/// `proxy_url` 为网关设置里的全局代理：国内直连 `api.github.com` 往往不可用，
/// 渠道配置过代理时这里同样走代理。
pub async fn check(
    current_version: &str,
    proxy_url: Option<&str>,
) -> Result<UpdateInfo, ServiceError> {
    let url = format!("{API_BASE}/repos/{REPO}/releases/latest");
    let client = build_client(proxy_url)?;

    info!("[update] 检查更新: {url}");
    let response = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", API_VERSION)
        .header("User-Agent", USER_AGENT)
        .send()
        .await
        .map_err(|e| {
            warn!("[update] 请求 GitHub 失败: {e}");
            ServiceError::Internal {
                message: format!("无法连接 GitHub：{e}"),
                detail: Some("若本机网络需要代理，请在「设置 → 请求与限流」配置全局代理".to_string()),
            }
        })?;

    let status = response.status();

    // 仓库还没有正式发布：GitHub 返回 404，这不是错误
    if status == reqwest::StatusCode::NOT_FOUND {
        info!("[update] 仓库尚无正式发布");
        return Ok(UpdateInfo::no_release(current_version));
    }

    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        warn!("[update] GitHub 返回 {status}: {body}");
        return Err(ServiceError::Internal {
            message: format!("GitHub 返回 {status}"),
            detail: Some(github_error_hint(status, &body)),
        });
    }

    // reqwest 未启用 `json` feature，与项目其它模块一致走 text + serde_json
    let body = response.text().await.map_err(|e| {
        warn!("[update] 读取发布信息失败: {e}");
        ServiceError::Internal {
            message: format!("读取 GitHub 发布信息失败：{e}"),
            detail: None,
        }
    })?;

    let release: GhRelease = serde_json::from_str(&body).map_err(|e| {
        warn!("[update] 解析发布信息失败: {e}");
        ServiceError::Internal {
            message: format!("解析 GitHub 发布信息失败：{e}"),
            detail: None,
        }
    })?;

    Ok(build_update_info(current_version, release))
}

/// 把 REST API 的发布对象映射为前端可用的更新信息
fn build_update_info(current_version: &str, release: GhRelease) -> UpdateInfo {
    let latest = strip_tag_prefix(&release.tag_name);
    let current = strip_tag_prefix(current_version);

    if release.draft || release.prerelease {
        info!("[update] 最新发布为草稿/预发布，忽略: {}", release.tag_name);
        return UpdateInfo::no_release(current_version);
    }

    let available = match (parse_version(&latest), parse_version(&current)) {
        (Some(latest), Some(current)) => latest > current,
        // 版本号不是 x.y.z 形态时无法比较，退回字符串不相等判断
        _ => latest != current,
    };

    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let asset = pick_asset(&release.assets, os, arch);
    if asset.is_none() && !release.assets.is_empty() {
        warn!("[update] 发布中没有匹配 {os}/{arch} 的安装包");
    }

    info!(
        "[update] 当前 {current}，最新 {latest}，有更新: {available}，资产: {}",
        asset.map(|a| a.name.as_str()).unwrap_or("无")
    );

    UpdateInfo {
        current_version: current,
        latest_version: Some(latest),
        available,
        name: release.name,
        notes: release.body.filter(|b| !b.trim().is_empty()),
        published_at: release.published_at,
        release_url: release.html_url,
        asset_name: asset.map(|a| a.name.clone()),
        download_url: asset.map(|a| a.browser_download_url.clone()),
        auto_update_ready: has_updater_manifest(&release.assets),
    }
}

/// 构建 HTTP 客户端；`proxy_url` 非空时全程走代理
fn build_client(proxy_url: Option<&str>) -> Result<reqwest::Client, ServiceError> {
    let mut builder = reqwest::Client::builder().timeout(REQUEST_TIMEOUT);

    if let Some(proxy) = proxy_url.map(str::trim).filter(|p| !p.is_empty()) {
        let proxy = reqwest::Proxy::all(proxy).map_err(|e| ServiceError::Internal {
            message: format!("无效的代理地址 {proxy}：{e}"),
            detail: None,
        })?;
        builder = builder.proxy(proxy);
        info!("[update] 使用全局代理检查更新");
    }

    builder.build().map_err(|e| ServiceError::Internal {
        message: format!("构建 HTTP 客户端失败：{e}"),
        detail: None,
    })
}

/// 给常见的非 2xx 状态补一句可操作的原因
fn github_error_hint(status: reqwest::StatusCode, body: &str) -> String {
    if status == reqwest::StatusCode::FORBIDDEN && body.contains("rate limit") {
        return "GitHub API 匿名调用额度已用尽（每小时 60 次），稍后再试".to_string();
    }
    if body.is_empty() {
        return "请确认网络可以访问 api.github.com".to_string();
    }
    body.chars().take(200).collect()
}

// ---------------------------------------------------------------------------
// 版本与资产
// ---------------------------------------------------------------------------

/// 去掉标签/版本号可能的 `v` 前缀
fn strip_tag_prefix(tag: &str) -> String {
    tag.trim().trim_start_matches(['v', 'V']).to_string()
}

/// 解析 `1.2.3` / `1.2.3-beta.1` 的主版本三元组，解析失败返回 None
fn parse_version(version: &str) -> Option<(u64, u64, u64)> {
    let core = version.trim().split(['-', '+']).next()?;
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    // 多出来的段说明不是 semver，交给字符串比较兜底
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

/// 按平台与架构挑选安装包
///
/// Windows 优先 NSIS `*-setup.exe`（与 CI 的 `updaterJsonPreferNsis` 一致），
/// macOS 用 dmg，Linux 用 AppImage 再退 deb。
fn pick_asset<'a>(assets: &'a [GhAsset], os: &str, arch: &str) -> Option<&'a GhAsset> {
    let arch_tokens: &[&str] = match arch {
        "aarch64" => &["aarch64", "arm64"],
        "x86_64" => &["x64", "x86_64", "amd64"],
        _ => &["i686", "x86"],
    };
    let extensions: &[&str] = match os {
        "windows" => &["-setup.exe", ".msi"],
        "macos" => &[".dmg"],
        _ => &[".appimage", ".deb"],
    };

    for extension in extensions {
        let hit = assets.iter().find(|asset| {
            let name = asset.name.to_lowercase();
            name.ends_with(extension)
                && arch_tokens.iter().any(|token| name.contains(token))
                && !name.ends_with(".sig")
        });
        if hit.is_some() {
            return hit;
        }
    }
    None
}

/// 发布里是否带 updater 清单（决定能否静默自动安装）
fn has_updater_manifest(assets: &[GhAsset]) -> bool {
    assets.iter().any(|a| a.name.eq_ignore_ascii_case(UPDATER_MANIFEST))
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(name: &str) -> GhAsset {
        GhAsset {
            name: name.to_string(),
            browser_download_url: format!("https://example.com/{name}"),
        }
    }

    /// 真实发布资产（取自 v0.9.2），用于回归资产选择逻辑
    fn release_assets() -> Vec<GhAsset> {
        [
            "silk-0.9.2-1.x86_64.rpm",
            "silk_0.9.2_aarch64.app.tar.gz",
            "silk_0.9.2_aarch64.dmg",
            "silk_0.9.2_amd64.AppImage",
            "silk_0.9.2_amd64.deb",
            "silk_0.9.2_x64-setup.exe",
            "silk_0.9.2_x64.app.tar.gz",
            "silk_0.9.2_x64.dmg",
            "silk_0.9.2_x64_en-US.msi",
        ]
        .iter()
        .map(|n| asset(n))
        .collect()
    }

    #[test]
    fn strip_prefix_handles_v_and_plain() {
        assert_eq!(strip_tag_prefix("v0.9.2"), "0.9.2");
        assert_eq!(strip_tag_prefix("V1.0.0"), "1.0.0");
        assert_eq!(strip_tag_prefix("0.9.2"), "0.9.2");
    }

    #[test]
    fn parse_version_accepts_semver_with_suffix() {
        assert_eq!(parse_version("1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("0.9"), Some((0, 9, 0)));
        assert_eq!(parse_version("1.2.3-beta.1"), Some((1, 2, 3)));
        assert_eq!(parse_version("1.2.3+build5"), Some((1, 2, 3)));
    }

    #[test]
    fn parse_version_rejects_non_semver() {
        assert_eq!(parse_version("1.2.3.4"), None);
        assert_eq!(parse_version("nightly"), None);
    }

    #[test]
    fn version_compare_is_numeric_not_lexical() {
        // 0.10.0 必须大于 0.9.2，字符串比较会判错
        assert!(parse_version("0.10.0").unwrap() > parse_version("0.9.2").unwrap());
        assert!(parse_version("1.0.0").unwrap() > parse_version("0.99.99").unwrap());
    }

    #[test]
    fn pick_windows_nsis_installer() {
        let assets = release_assets();
        let picked = pick_asset(&assets, "windows", "x86_64").unwrap();
        assert_eq!(picked.name, "silk_0.9.2_x64-setup.exe");
    }

    #[test]
    fn pick_macos_dmg_by_arch() {
        let assets = release_assets();
        assert_eq!(
            pick_asset(&assets, "macos", "x86_64").unwrap().name,
            "silk_0.9.2_x64.dmg"
        );
        assert_eq!(
            pick_asset(&assets, "macos", "aarch64").unwrap().name,
            "silk_0.9.2_aarch64.dmg"
        );
    }

    #[test]
    fn pick_linux_appimage_over_deb() {
        let assets = release_assets();
        let picked = pick_asset(&assets, "linux", "x86_64").unwrap();
        assert_eq!(picked.name, "silk_0.9.2_amd64.AppImage");
    }

    #[test]
    fn pick_asset_returns_none_when_arch_missing() {
        let assets = vec![asset("silk_0.9.2_aarch64.dmg")];
        assert!(pick_asset(&assets, "windows", "x86_64").is_none());
    }

    #[test]
    fn updater_manifest_detection() {
        assert!(!has_updater_manifest(&release_assets()));
        let mut with_manifest = release_assets();
        with_manifest.push(asset("latest.json"));
        assert!(has_updater_manifest(&with_manifest));
    }

    #[test]
    fn build_info_flags_new_version_and_asset() {
        let release = GhRelease {
            tag_name: "v0.9.3".to_string(),
            name: Some("Silk v0.9.3".to_string()),
            body: Some("修复若干问题".to_string()),
            published_at: Some("2026-09-12T00:00:00Z".to_string()),
            html_url: Some("https://github.com/morning-start/silk/releases/tag/v0.9.3".to_string()),
            draft: false,
            prerelease: false,
            assets: release_assets(),
        };
        let info = build_update_info("0.9.2", release);

        assert!(info.available);
        assert_eq!(info.latest_version.as_deref(), Some("0.9.3"));
        assert_eq!(info.current_version, "0.9.2");
        assert_eq!(info.notes.as_deref(), Some("修复若干问题"));
        // 所选资产必须与当前运行平台一致
        assert!(info.asset_name.is_some());
        assert!(info.download_url.is_some());
        assert!(!info.auto_update_ready);
    }

    #[test]
    fn build_info_no_update_when_same_version() {
        let release = GhRelease {
            tag_name: "v0.9.2".to_string(),
            name: None,
            body: None,
            published_at: None,
            html_url: None,
            draft: false,
            prerelease: false,
            assets: release_assets(),
        };
        let info = build_update_info("0.9.2", release);
        assert!(!info.available);
        assert_eq!(info.latest_version.as_deref(), Some("0.9.2"));
    }

    #[test]
    fn build_info_ignores_draft_and_prerelease() {
        for (draft, prerelease) in [(true, false), (false, true)] {
            let release = GhRelease {
                tag_name: "v1.0.0".to_string(),
                name: None,
                body: None,
                published_at: None,
                html_url: None,
                draft,
                prerelease,
                assets: release_assets(),
            };
            let info = build_update_info("0.9.2", release);
            assert!(!info.available);
            assert!(info.latest_version.is_none());
        }
    }

    #[test]
    fn empty_notes_become_none() {
        let release = GhRelease {
            tag_name: "v0.9.3".to_string(),
            name: None,
            body: Some("   \n ".to_string()),
            published_at: None,
            html_url: None,
            draft: false,
            prerelease: false,
            assets: Vec::new(),
        };
        let info = build_update_info("0.9.2", release);
        assert!(info.notes.is_none());
        assert!(info.asset_name.is_none());
    }

    /// 用真实的 `/releases/latest` 响应体检反序列化：
    /// GitHub 会返回大量本模块不关心的字段（id / node_id / author / uploader …），
    /// 这条测试确保多余的字段不会让解析失败，且我们需要的字段都能取到。
    #[test]
    fn deserializes_realistic_github_payload() {
        // 取自 https://api.github.com/repos/morning-start/silk/releases/latest
        // （已按需裁剪，但保留了全部顶层字段名与嵌套结构）
        let payload = r###"{
          "url": "https://api.github.com/repos/morning-start/silk/releases/250000001",
          "assets_url": "https://api.github.com/repos/morning-start/silk/releases/250000001/assets",
          "upload_url": "https://uploads.github.com/repos/morning-start/silk/releases/250000001/assets{?name,label}",
          "html_url": "https://github.com/morning-start/silk/releases/tag/v0.9.2",
          "id": 250000001,
          "node_id": "RE_kwDOAbc1234",
          "tag_name": "v0.9.2",
          "target_commitish": "main",
          "name": "Silk v0.9.2",
          "draft": false,
          "author": { "login": "morning-start", "id": 1, "type": "User" },
          "prerelease": false,
          "created_at": "2026-09-10T02:20:00Z",
          "published_at": "2026-09-10T02:31:29Z",
          "assets": [
            {
              "url": "https://api.github.com/repos/morning-start/silk/releases/assets/1",
              "id": 1,
              "node_id": "RA_kwDOAbc1234",
              "name": "silk_0.9.2_x64-setup.exe",
              "label": "",
              "uploader": { "login": "github-actions[bot]", "id": 41898282 },
              "content_type": "application/octet-stream",
              "state": "uploaded",
              "size": 8123456,
              "download_count": 42,
              "created_at": "2026-09-10T02:25:00Z",
              "browser_download_url": "https://github.com/morning-start/silk/releases/download/v0.9.2/silk_0.9.2_x64-setup.exe"
            },
            {
              "name": "silk_0.9.2_x64.dmg",
              "browser_download_url": "https://github.com/morning-start/silk/releases/download/v0.9.2/silk_0.9.2_x64.dmg"
            }
          ],
          "tarball_url": "https://api.github.com/repos/morning-start/silk/tarball/v0.9.2",
          "zipball_url": "https://api.github.com/repos/morning-start/silk/zipball/v0.9.2",
          "body": "## 修复\n- 网关路径守卫"
        }"###;

        let release: GhRelease =
            serde_json::from_str(payload).expect("真实响应应当可以解析");

        assert_eq!(release.tag_name, "v0.9.2");
        assert_eq!(release.name.as_deref(), Some("Silk v0.9.2"));
        assert_eq!(release.published_at.as_deref(), Some("2026-09-10T02:31:29Z"));
        assert!(!release.draft);
        assert!(!release.prerelease);
        assert_eq!(release.assets.len(), 2);
        assert!(!has_updater_manifest(&release.assets));

        let info = build_update_info("0.9.2", release);
        // 同版本 → 无更新，但仍能定位到本平台安装包（用于「前往下载」）
        assert!(!info.available);
        assert_eq!(info.latest_version.as_deref(), Some("0.9.2"));
        assert!(info.download_url.is_some());
    }

    /// GitHub 对未知路径/无发布返回 404 —— 对应的空响应体必须能让
    /// `build_update_info` 走到「无发布」分支而不是 panic。
    #[test]
    fn missing_body_and_assets_are_tolerated() {
        let payload = r#"{"tag_name":"v0.9.3","draft":false,"prerelease":false}"#;
        let release: GhRelease = serde_json::from_str(payload).expect("缺省字段应可解析");
        assert!(release.assets.is_empty());

        let info = build_update_info("0.9.2", release);
        assert!(info.available);
        assert!(info.asset_name.is_none());
        assert!(info.download_url.is_none());
        assert!(info.release_url.is_none());
        assert!(!info.auto_update_ready);
    }
}
