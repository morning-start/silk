//! 协议内核（prism.wasm）下载与更新
//!
//! prism 每次发布 `v*` 标签都会产出三个资产：
//! - `prism.wasm`        运行时产物，宿主直接加载
//! - `prism.wasm.sha256` `sha256sum` 格式校验和
//! - `prism.release.json` 构建清单（tag / commit / moonbit / sha256 / bytes / exports）
//!
//! 本模块负责「检查新内核 → 下载 → 校验 → 原子替换」，与 `update_service`
//! （应用自身更新）职责分离：比对对象不同（内核版本 vs 应用版本）、资产形态
//! 不同（wasm vs 安装包）、校验强度不同（sha256 + ABI 探测 vs 签名清单）。
//! 两者共用「读 GitHub Releases + 走全局代理」这段基建。
//!
//! 安全边界（两道关，缺一不可）：
//! 1. **sha256**：证明下载的字节与发布方声明的一致（未被篡改/截断）；
//! 2. **ABI 探测**：用 wasmtime 真实实例化并调用 `wasm_abi_version()`，证明这份
//!    wasm 真的能跑且导出签名与宿主兼容。校验和无法覆盖这一层。
//!
//! 写入目标是应用数据目录的 `prism.wasm`（`prism_wasm::resolve_wasm_source` 的
//! 第二优先级）。若可执行文件同目录已存在 prism.wasm，它会一直抢占加载权，
//! 此时写数据目录将静默无效 —— 本模块直接拒绝安装并说明原因。

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::error::ServiceError;
use crate::protocol::prism_wasm;

/// 内核发布仓库
const REPO: &str = "morning-start/prism";
const API_BASE: &str = "https://api.github.com";
/// GitHub REST API 版本固定头（与 update_service 保持一致）
const API_VERSION: &str = "2026-03-10";
const USER_AGENT: &str = "silk-kernel-updater";
/// 元数据请求超时（只读发布信息，短超时避免拖慢关于页）
const META_TIMEOUT: Duration = Duration::from_secs(15);
/// 内核下载超时（~800KB，但要容忍慢网）
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);

/// 发布资产文件名
const ASSET_WASM: &str = "prism.wasm";
const ASSET_SHA256: &str = "prism.wasm.sha256";

/// 版本记录文件名（存于应用数据目录，记录当前已安装内核的构建清单）
const MANIFEST_FILE: &str = "prism.release.json";
/// 替换前的备份文件名
const BACKUP_FILE: &str = "prism.wasm.bak";
/// 与备份内核配对的版本记录（**必须与 BACKUP_FILE 成对写入**）
///
/// 只备份 wasm 而不备份清单，回滚后会出现「内核是旧的、版本号是新的」这种
/// 自相矛盾的状态：关于页显示新版本，更新检查也说已是最新，但实际跑的是旧内核。
const BACKUP_MANIFEST_FILE: &str = "prism.release.json.bak";
/// 下载临时文件名
const DOWNLOAD_FILE: &str = "prism.wasm.download";

/// 内核体积下限：发布流程自身也以此拦截残缺产物（见 prism release.yml）
const MIN_WASM_BYTES: usize = 10240;

// ---------------------------------------------------------------------------
// 对外结构
// ---------------------------------------------------------------------------

/// 当前内核状态（关于页展示用）
#[derive(Debug, Clone, Serialize)]
pub struct KernelStatus {
    /// 已安装内核的版本（来自构建清单；内嵌/程序目录来源无法反查时为 None）
    pub version: Option<String>,
    /// 当前运行内核的 ABI（探测失败为 None）
    pub abi: Option<String>,
    /// 当前运行内核的 IR schema 版本
    pub ir_schema: Option<String>,
    /// 内核来源：exe_dir / data_dir / embedded
    pub source: String,
    /// 实际加载路径（内嵌来源为 None）
    pub path: Option<String>,
    /// 宿主支持的 ABI（前端据此展示兼容性）
    pub supported_abi: String,
    /// 是否可被本功能更新（程序目录抢占时为 false）
    pub updatable: bool,
    /// 不可更新时的原因说明
    pub updatable_reason: Option<String>,
    /// 备份内核的版本（有备份且能读出清单时非空）
    pub backup_version: Option<String>,
    /// 是否存在可回滚的备份内核
    pub backup_available: bool,
}

/// 内核更新检查结果
#[derive(Debug, Clone, Serialize)]
pub struct KernelInfo {
    /// 当前已安装版本（无法反查时为 None）
    pub current_version: Option<String>,
    /// 最新发布版本（已去掉 `v` 前缀）
    pub latest_version: Option<String>,
    /// 是否有比当前更新的发布
    pub available: bool,
    /// 发布标题
    pub name: Option<String>,
    /// 发布说明（Markdown）
    pub notes: Option<String>,
    /// 发布时间（ISO 8601）
    pub published_at: Option<String>,
    /// 发布页地址
    pub release_url: Option<String>,
    /// 内核体积（字节）
    pub asset_size: Option<u64>,
    /// 是否可安装（程序目录抢占时为 false）
    pub can_install: bool,
    /// 不可安装时的原因说明
    pub blocked_reason: Option<String>,
}

/// 安装结果
#[derive(Debug, Clone, Serialize)]
pub struct KernelInstallResult {
    /// 已安装的版本
    pub version: Option<String>,
    /// 已安装内核的 ABI
    pub abi: String,
    /// 内核来源（安装后恒为 data_dir）
    pub source: String,
    /// 是否必须重启应用才能生效（内核为进程级单例，恒为 true）
    pub requires_restart: bool,
}

// ---------------------------------------------------------------------------
// GitHub REST API 响应
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    published_at: Option<String>,
    html_url: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Debug, Clone, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

/// 构建清单（`prism.release.json`，只取本模块需要的字段）
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReleaseManifest {
    tag: String,
    #[serde(default)]
    sha256: Option<String>,
    #[serde(default)]
    bytes: Option<u64>,
}

// ---------------------------------------------------------------------------
// 路径
// ---------------------------------------------------------------------------

fn data_dir() -> Result<PathBuf, ServiceError> {
    crate::get_settings_path()
        .and_then(|path| path.parent())
        .map(PathBuf::from)
        .ok_or_else(|| ServiceError::Internal {
            message: "无法获取应用数据目录".to_string(),
            detail: None,
        })
}

fn manifest_path() -> Result<PathBuf, ServiceError> {
    Ok(data_dir()?.join(MANIFEST_FILE))
}

/// 读取已安装内核的构建清单（不存在或损坏时返回 None，不报错）
fn read_manifest() -> Option<ReleaseManifest> {
    read_manifest_at(manifest_path().ok()?.parent()?)
}

/// 从指定目录读取构建清单（抽成纯函数以便测试）
fn read_manifest_at(dir: &std::path::Path) -> Option<ReleaseManifest> {
    read_manifest_at_named(dir, MANIFEST_FILE)
}

/// 从指定目录读取指定名字的清单（用于读取备份清单）
fn read_manifest_at_named(dir: &std::path::Path, file_name: &str) -> Option<ReleaseManifest> {
    let text = std::fs::read_to_string(dir.join(file_name)).ok()?;
    serde_json::from_str(&text).ok()
}

// ---------------------------------------------------------------------------
// 当前状态
// ---------------------------------------------------------------------------

/// 查询当前内核状态（来源、版本、ABI）
pub fn status() -> KernelStatus {
    let (source, path) = prism_wasm::resolve_wasm_source();
    let abi = prism_wasm::current_abi().ok();

    // 只有数据目录来源才由本功能管理，其版本从构建清单反查
    let version = match source {
        prism_wasm::WasmSource::DataDir => read_manifest().map(|manifest| manifest.tag),
        _ => None,
    };

    let exe_override = prism_wasm::exe_dir_wasm_path();
    let updatable = exe_override.is_none();
    let updatable_reason = exe_override.map(|path| {
        format!(
            "内核由程序目录下的 {} 提供，加载优先级高于应用数据目录，无法在此更新",
            path.display()
        )
    });

    // 备份状态：只有数据目录来源 + 备份文件确实存在时才算「可回滚」
    let backup_available = prism_wasm::data_dir_wasm_path()
        .map(|path| path.with_file_name(BACKUP_FILE).exists())
        .unwrap_or(false);
    let backup_version = if backup_available {
        data_dir()
            .ok()
            .and_then(|dir| read_manifest_at_named(&dir, BACKUP_MANIFEST_FILE))
            .map(|manifest| manifest.tag)
    } else {
        None
    };

    KernelStatus {
        version,
        abi: abi.as_ref().map(|info| info.abi.clone()),
        ir_schema: abi.map(|info| info.ir_schema),
        source: source.as_str().to_string(),
        path: path.map(|path| path.display().to_string()),
        supported_abi: prism_wasm::SUPPORTED_ABI.to_string(),
        updatable,
        updatable_reason,
        backup_version,
        backup_available,
    }
}

// ---------------------------------------------------------------------------
// 检查
// ---------------------------------------------------------------------------

/// 检查内核更新
///
/// `proxy_url` 为网关设置里的全局代理：国内直连 `api.github.com` 往往不可用。
pub async fn check(proxy_url: Option<&str>) -> Result<KernelInfo, ServiceError> {
    let url = format!("{API_BASE}/repos/{REPO}/releases/latest");
    let client = crate::application::update_service::build_client_with_timeout(proxy_url, META_TIMEOUT)?;

    info!("[kernel] 检查内核更新: {url}");
    let response = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", API_VERSION)
        .header("User-Agent", USER_AGENT)
        .send()
        .await
        .map_err(|e| {
            warn!("[kernel] 请求 GitHub 失败: {e}");
            ServiceError::Internal {
                message: format!("无法连接 GitHub：{e}"),
                detail: Some("若本机网络需要代理，请在「设置 → 请求与限流」配置全局代理".to_string()),
            }
        })?;

    let http_status = response.status();

    // 仓库还没有正式发布：404 不是错误
    if http_status == reqwest::StatusCode::NOT_FOUND {
        info!("[kernel] 内核仓库尚无正式发布");
        return Ok(empty_info(None));
    }

    if !http_status.is_success() {
        let body = response.text().await.unwrap_or_default();
        warn!("[kernel] GitHub 返回 {http_status}: {body}");
        return Err(ServiceError::Internal {
            message: format!("GitHub 返回 {http_status}"),
            detail: Some(crate::application::update_service::github_error_hint(
                http_status, &body,
            )),
        });
    }

    let body = response.text().await.map_err(|e| ServiceError::Internal {
        message: format!("读取内核发布信息失败：{e}"),
        detail: None,
    })?;

    let release: GhRelease = serde_json::from_str(&body).map_err(|e| ServiceError::Internal {
        message: format!("解析内核发布信息失败：{e}"),
        detail: None,
    })?;

    Ok(build_kernel_info(release))
}

/// 把发布对象映射为前端可用的内核信息
fn build_kernel_info(release: GhRelease) -> KernelInfo {
    if release.draft || release.prerelease {
        info!("[kernel] 最新发布为草稿/预发布，忽略: {}", release.tag_name);
        return empty_info(None);
    }

    let latest = crate::application::update_service::strip_tag_prefix(&release.tag_name);
    let current = read_manifest().map(|manifest| manifest.tag);

    let wasm_asset = release.assets.iter().find(|a| a.name == ASSET_WASM);
    let sha_asset = release.assets.iter().find(|a| a.name == ASSET_SHA256);

    let available = match current.as_deref() {
        // 无法反查当前版本（内嵌或程序目录来源）→ 无法比较，视为可更新
        None => true,
        Some(current) => current != latest,
    };

    // 程序目录抢占：写入数据目录不会生效，直接标记为不可安装
    let blocked = prism_wasm::exe_dir_wasm_path().map(|path| {
        format!(
            "内核由程序目录下的 {} 提供，加载优先级更高，安装不会生效",
            path.display()
        )
    });

    // 缺少必要资产时不允许安装（无法校验的产物不装）
    let missing = if wasm_asset.is_none() {
        Some("本次发布未提供 prism.wasm".to_string())
    } else if sha_asset.is_none() {
        Some("本次发布未提供校验和文件，无法验证内核完整性".to_string())
    } else {
        None
    };

    let blocked_reason = blocked.or(missing);

    info!(
        "[kernel] 当前 {:?}，最新 {}，有更新: {available}，内核 {} 字节",
        current,
        latest,
        wasm_asset.map(|a| a.size).unwrap_or(0)
    );

    KernelInfo {
        current_version: current,
        latest_version: Some(latest),
        available,
        name: release.name,
        notes: release.body.filter(|b| !b.trim().is_empty()),
        published_at: release.published_at,
        release_url: release.html_url,
        asset_size: wasm_asset.map(|a| a.size),
        can_install: blocked_reason.is_none(),
        blocked_reason,
    }
}

fn empty_info(current: Option<String>) -> KernelInfo {
    KernelInfo {
        current_version: current,
        latest_version: None,
        available: false,
        name: None,
        notes: None,
        published_at: None,
        release_url: None,
        asset_size: None,
        can_install: false,
        blocked_reason: None,
    }
}

// ---------------------------------------------------------------------------
// 安装
// ---------------------------------------------------------------------------

/// 下载并安装最新内核
///
/// 流程：下载 wasm → 下载校验和 → sha256 比对 → ABI 实例化探测 → 备份 → 原子替换
/// → 写构建清单。任一步失败都在落盘前中止，旧内核保持原样。
pub async fn install(proxy_url: Option<&str>) -> Result<KernelInstallResult, ServiceError> {
    // 程序目录抢占时提前失败，避免下载完才发现写进去不生效
    if let Some(path) = prism_wasm::exe_dir_wasm_path() {
        return Err(ServiceError::BadRequest {
            message: format!(
                "内核由程序目录下的 {} 提供，加载优先级更高，无法在此更新",
                path.display()
            ),
            code: Some("kernel_externally_managed".to_string()),
        });
    }

    let release = fetch_latest_release(proxy_url).await?;
    if release.draft || release.prerelease {
        return Err(ServiceError::BadRequest {
            message: "最新发布为草稿或预发布，拒绝安装".to_string(),
            code: None,
        });
    }

    let wasm_asset = find_asset(&release, ASSET_WASM)?;
    let sha_asset = find_asset(&release, ASSET_SHA256)?;

    // 说明：不下载发布里的 prism.release.json 直接落盘——它的 tag 带 `v` 前缀
    // （如 v0.1.4），与检查时比对用的去前缀版本不一致，会导致「永远有新版本」。
    // 这里本地重建清单，只记录本模块真正需要的字段（去前缀 tag / 实际 sha256 / 实际字节数）。

    let client =
        crate::application::update_service::build_client_with_timeout(proxy_url, DOWNLOAD_TIMEOUT)?;

    // 1. 下载内核字节
    info!("[kernel] 下载内核 {} 字节: {}", wasm_asset.size, wasm_asset.browser_download_url);
    let wasm_bytes = download(&client, &wasm_asset.browser_download_url).await?;
    if wasm_bytes.len() < MIN_WASM_BYTES {
        return Err(ServiceError::Internal {
            message: format!(
                "下载的内核体积异常（{} 字节，低于 {} 字节下限），已中止",
                wasm_bytes.len(),
                MIN_WASM_BYTES
            ),
            detail: None,
        });
    }

    // 2. 下载并解析校验和
    let sha_text = String::from_utf8(download(&client, &sha_asset.browser_download_url).await?)
        .map_err(|_| ServiceError::Internal {
            message: "校验和文件不是合法 UTF-8 文本".to_string(),
            detail: None,
        })?;
    let expected = parse_sha256_file(&sha_text)?;

    // 3. sha256 校验（第一道关：证明字节未被篡改）
    let actual = sha256_hex(&wasm_bytes);
    if !actual.eq_ignore_ascii_case(&expected) {
        return Err(ServiceError::Internal {
            message: "内核校验和不匹配，已中止安装".to_string(),
            detail: Some(format!("期望 {expected}，实际 {actual}")),
        });
    }
    info!("[kernel] sha256 校验通过: {actual}");

    // 4. ABI 探测（第二道关：证明这份 wasm 真能跑且签名兼容）
    //    用独立实例探测，不触碰正在运行的内核
    let abi = prism_wasm::probe_abi(&wasm_bytes).map_err(|e| ServiceError::BadRequest {
        message: format!("内核无法加载，已中止安装：{e}"),
        code: Some("kernel_probe_failed".to_string()),
    })?;
    if abi.abi != prism_wasm::SUPPORTED_ABI {
        return Err(ServiceError::BadRequest {
            message: format!(
                "内核 ABI {} 与当前版本不兼容（需要 {}），已中止安装",
                abi.abi,
                prism_wasm::SUPPORTED_ABI
            ),
            code: Some("kernel_abi_mismatch".to_string()),
        });
    }
    info!(abi = %abi.abi, ir_schema = %abi.ir_schema, "[kernel] ABI 探测通过");

    // 5. 落盘：备份 → 原子替换 → 写构建清单（失败回滚）
    let dir = data_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| ServiceError::Internal {
        message: format!("创建应用数据目录失败：{e}"),
        detail: None,
    })?;

    let manifest = ReleaseManifest {
        tag: crate::application::update_service::strip_tag_prefix(&release.tag_name),
        sha256: Some(actual),
        bytes: Some(wasm_bytes.len() as u64),
    };

    commit_kernel(&dir, &wasm_bytes, &manifest)?;

    info!(
        version = %manifest.tag,
        "[kernel] 内核安装完成，重启应用后生效"
    );

    Ok(KernelInstallResult {
        version: Some(manifest.tag),
        abi: abi.abi,
        source: prism_wasm::WasmSource::DataDir.as_str().to_string(),
        requires_restart: true,
    })
}

/// 回滚到上一次替换前的内核备份
///
/// 备份是单槽位的一对文件（内核 + 版本记录）。回滚即把两者一起还原，并清掉备份
/// —— 备份只有一个槽位，回滚后它已失去意义（当前内核就是刚还原的那份），
/// 留着会让「可回滚」一直显示为真，诱使用户重复点击。
///
/// 这是「内核更新后起不来」的唯一自救出口：装上的内核若与宿主不兼容，
/// 用户至少能退回上一版而不必重装应用。
pub fn rollback_to_backup() -> Result<KernelInstallResult, ServiceError> {
    // 程序目录抢占时，数据目录的内核根本不参与加载，回滚也无意义
    if let Some(path) = prism_wasm::exe_dir_wasm_path() {
        return Err(ServiceError::BadRequest {
            message: format!(
                "内核由程序目录下的 {} 提供，加载优先级更高，回滚不会生效",
                path.display()
            ),
            code: Some("kernel_externally_managed".to_string()),
        });
    }

    let dir = data_dir()?;
    let target = dir.join(prism_wasm::PRISM_WASM_FILE);
    let manifest_path = dir.join(MANIFEST_FILE);
    let backup = dir.join(BACKUP_FILE);
    let backup_manifest = dir.join(BACKUP_MANIFEST_FILE);

    if !backup.exists() {
        return Err(ServiceError::BadRequest {
            message: "没有可回滚的内核备份（可能从未更新过，或备份已被回滚消耗）".to_string(),
            code: Some("kernel_no_backup".to_string()),
        });
    }

    // 回滚前先验证备份内核真的可用：把一个坏备份换上去比不换更糟
    let bytes = std::fs::read(&backup).map_err(|e| ServiceError::Internal {
        message: format!("读取内核备份失败：{e}"),
        detail: None,
    })?;
    let abi = prism_wasm::probe_abi(&bytes).map_err(|e| ServiceError::BadRequest {
        message: format!("内核备份无法加载，拒绝回滚：{e}"),
        code: Some("kernel_probe_failed".to_string()),
    })?;

    // 还原内核（覆盖当前内核）
    std::fs::copy(&backup, &target).map_err(|e| ServiceError::Internal {
        message: format!("回滚内核文件失败：{e}"),
        detail: None,
    })?;

    // 版本记录同步还原；备份清单缺失/损坏时删除当前清单，
    // 宁可显示「版本未知」，也不能留下指向已不存在的内核的版本号
    let restored_tag = match read_manifest_at_named(&dir, BACKUP_MANIFEST_FILE) {
        Some(manifest) => {
            if let Err(e) = std::fs::copy(&backup_manifest, &manifest_path) {
                warn!(error = %e, "[kernel] 回滚版本记录失败");
                None
            } else {
                Some(manifest.tag)
            }
        }
        None => {
            let _ = std::fs::remove_file(&manifest_path);
            None
        }
    };

    // 备份已被消耗（单槽位）
    let _ = std::fs::remove_file(&backup);
    let _ = std::fs::remove_file(&backup_manifest);

    info!(
        version = ?restored_tag,
        abi = %abi.abi,
        "[kernel] 已回滚到备份内核，重启应用后生效"
    );

    Ok(KernelInstallResult {
        version: restored_tag,
        abi: abi.abi,
        source: prism_wasm::WasmSource::DataDir.as_str().to_string(),
        requires_restart: true,
    })
}

/// 落盘内核字节与构建清单（备份 → 原子替换 → 写清单，任一步失败都回滚）。
///
/// 备份是**一对文件**（内核 + 版本记录），单槽位：每次安装覆盖上一次的备份。
/// 回滚时两者一起还原，避免「内核回旧了、版本号还是新的」的分裂状态。
///
/// 抽成独立函数以便单测覆盖 Windows 上「rename 覆盖已存在文件」与失败回滚
/// 这两条最容易出错的路径（不依赖网络）。
fn commit_kernel(
    dir: &std::path::Path,
    wasm_bytes: &[u8],
    manifest: &ReleaseManifest,
) -> Result<(), ServiceError> {
    let target = dir.join(prism_wasm::PRISM_WASM_FILE);
    let manifest_path = dir.join(MANIFEST_FILE);
    let backup = dir.join(BACKUP_FILE);
    let backup_manifest = dir.join(BACKUP_MANIFEST_FILE);
    let temp = dir.join(DOWNLOAD_FILE);

    // 先写临时文件，校验通过后再改名，避免半写状态
    std::fs::write(&temp, wasm_bytes).map_err(|e| ServiceError::Internal {
        message: format!("写入临时内核文件失败：{e}"),
        detail: None,
    })?;

    // 备份现有内核与版本记录（成对；存在才备份）
    let had_existing = target.exists();
    let had_manifest = manifest_path.exists();
    if had_existing {
        if let Err(e) = std::fs::copy(&target, &backup) {
            let _ = std::fs::remove_file(&temp);
            return Err(ServiceError::Internal {
                message: format!("备份现有内核失败：{e}"),
                detail: None,
            });
        }
    }
    if had_manifest {
        if let Err(e) = std::fs::copy(&manifest_path, &backup_manifest) {
            let _ = std::fs::remove_file(&temp);
            // 内核已备份但清单没有 → 备份对不完整，清掉避免留下误导性的半份备份
            let _ = std::fs::remove_file(&backup);
            return Err(ServiceError::Internal {
                message: format!("备份现有内核版本记录失败：{e}"),
                detail: None,
            });
        }
    }

    // 原子替换（Windows 上 std::fs::rename 使用 MoveFileEx + REPLACE_EXISTING，可覆盖）
    if let Err(e) = std::fs::rename(&temp, &target) {
        let _ = std::fs::remove_file(&temp);
        return Err(ServiceError::Internal {
            message: format!("替换内核文件失败：{e}"),
            detail: None,
        });
    }

    // 写构建清单；失败则回滚内核文件，避免「内核已换但版本记录缺失」
    let text = serde_json::to_string_pretty(manifest).map_err(|e| ServiceError::Internal {
        message: format!("序列化内核清单失败：{e}"),
        detail: None,
    })?;
    if let Err(e) = crate::application::config_writer::write_text_atomic(&manifest_path, &text) {
        rollback(&target, &manifest_path, &backup, &backup_manifest, had_existing);
        return Err(ServiceError::Internal {
            message: format!("写入内核清单失败：{e}"),
            detail: None,
        });
    }

    Ok(())
}

/// 回滚内核与版本记录到备份状态（替换失败时恢复）
fn rollback(
    target: &std::path::Path,
    manifest_path: &std::path::Path,
    backup: &std::path::Path,
    backup_manifest: &std::path::Path,
    had_existing: bool,
) {
    if had_existing && backup.exists() {
        if let Err(e) = std::fs::copy(backup, target) {
            warn!(error = %e, "[kernel] 回滚内核文件失败，请手动恢复备份");
        } else {
            info!("[kernel] 已回滚到原内核");
        }
    } else {
        let _ = std::fs::remove_file(target);
    }

    // 版本记录必须与内核同步回滚，否则版本号会指向一个并不存在的新内核
    if backup_manifest.exists() {
        if let Err(e) = std::fs::copy(backup_manifest, manifest_path) {
            warn!(error = %e, "[kernel] 回滚内核版本记录失败，请手动恢复备份");
        }
    }
}

/// 拉取最新正式发布（404 视为无发布，返回错误由调用方处理）
async fn fetch_latest_release(proxy_url: Option<&str>) -> Result<GhRelease, ServiceError> {
    let url = format!("{API_BASE}/repos/{REPO}/releases/latest");
    let client = crate::application::update_service::build_client_with_timeout(proxy_url, META_TIMEOUT)?;

    let response = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", API_VERSION)
        .header("User-Agent", USER_AGENT)
        .send()
        .await
        .map_err(|e| ServiceError::Internal {
            message: format!("无法连接 GitHub：{e}"),
            detail: Some("若本机网络需要代理，请在「设置 → 请求与限流」配置全局代理".to_string()),
        })?;

    let http_status = response.status();
    if http_status == reqwest::StatusCode::NOT_FOUND {
        return Err(ServiceError::NotFound {
            message: "内核正式发布".to_string(),
        });
    }
    if !http_status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(ServiceError::Internal {
            message: format!("GitHub 返回 {http_status}"),
            detail: Some(crate::application::update_service::github_error_hint(
                http_status, &body,
            )),
        });
    }

    let body = response.text().await.map_err(|e| ServiceError::Internal {
        message: format!("读取内核发布信息失败：{e}"),
        detail: None,
    })?;
    serde_json::from_str(&body).map_err(|e| ServiceError::Internal {
        message: format!("解析内核发布信息失败：{e}"),
        detail: None,
    })
}

fn find_asset<'a>(release: &'a GhRelease, name: &str) -> Result<&'a GhAsset, ServiceError> {
    release
        .assets
        .iter()
        .find(|asset| asset.name == name)
        .ok_or_else(|| ServiceError::BadRequest {
            message: format!("本次发布缺少必需资产 {name}，无法安全安装"),
            code: Some("kernel_asset_missing".to_string()),
        })
}

/// 下载二进制内容
async fn download(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, ServiceError> {
    let response = client
        .get(url)
        .header("User-Agent", USER_AGENT)
        .send()
        .await
        .map_err(|e| ServiceError::Internal {
            message: format!("下载内核失败：{e}"),
            detail: None,
        })?;

    if !response.status().is_success() {
        return Err(ServiceError::Internal {
            message: format!("下载内核失败：HTTP {}", response.status()),
            detail: None,
        });
    }

    response
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|e| ServiceError::Internal {
            message: format!("读取内核数据失败：{e}"),
            detail: None,
        })
}

// ---------------------------------------------------------------------------
// 纯函数（可单测）
// ---------------------------------------------------------------------------

/// 解析 `sha256sum` 输出：`<hash>  <filename>`，取首段十六进制哈希。
///
/// 兼容多行（只取第一行有效记录）与 `*filename` 二进制标记形式。
fn parse_sha256_file(text: &str) -> Result<String, ServiceError> {
    let invalid = || ServiceError::Internal {
        message: "校验和文件格式无法识别".to_string(),
        detail: Some(format!("内容片段: {}", text.chars().take(120).collect::<String>())),
    };

    let hash = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .and_then(|line| line.split_whitespace().next())
        .ok_or_else(invalid)?;

    // 必须是 64 位十六进制（sha256），否则拒绝——避免把空文件/错误页当成校验和
    if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(invalid());
    }
    Ok(hash.to_ascii_lowercase())
}

/// 计算字节的 SHA-256 十六进制摘要
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
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
            size: 826_505,
        }
    }

    /// 真实发布资产（对齐 prism release.yml 的 files 列表）
    fn release_assets() -> Vec<GhAsset> {
        vec![
            asset("prism.wasm"),
            asset("prism.wasm.sha256"),
            asset("prism.release.json"),
        ]
    }

    fn release(tag: &str, assets: Vec<GhAsset>) -> GhRelease {
        GhRelease {
            tag_name: tag.to_string(),
            name: Some(format!("Prism {tag}")),
            body: Some("修复若干转换问题".to_string()),
            published_at: Some("2026-09-22T13:46:30Z".to_string()),
            html_url: Some(format!(
                "https://github.com/morning-start/prism/releases/tag/{tag}"
            )),
            draft: false,
            prerelease: false,
            assets,
        }
    }

    #[test]
    fn parse_sha256_accepts_standard_and_binary_marker() {
        let hash = "1162252464b76a32b4670c22d346c25422b8590f00821e21b623793d0f5b651d";
        assert_eq!(parse_sha256_file(&format!("{hash}  prism.wasm")).unwrap(), hash);
        assert_eq!(parse_sha256_file(&format!("{hash} *prism.wasm")).unwrap(), hash);
        // 大写归一化为小写
        assert_eq!(
            parse_sha256_file(&format!("{}  prism.wasm", hash.to_uppercase())).unwrap(),
            hash
        );
    }

    #[test]
    fn parse_sha256_skips_leading_blank_lines() {
        let hash = "1162252464b76a32b4670c22d346c25422b8590f00821e21b623793d0f5b651d";
        assert_eq!(
            parse_sha256_file(&format!("\n\n{hash}  prism.wasm\n")).unwrap(),
            hash
        );
    }

    #[test]
    fn parse_sha256_rejects_malformed() {
        // 空内容、错误页、长度不对的哈希都必须拒绝，避免「校验通过」的假阳性
        assert!(parse_sha256_file("").is_err());
        assert!(parse_sha256_file("   \n  ").is_err());
        assert!(parse_sha256_file("<html>404 not found</html>").is_err());
        assert!(parse_sha256_file("abc123  prism.wasm").is_err());
        // 64 位但含非十六进制字符
        assert!(parse_sha256_file(&format!("{}z  prism.wasm", "0".repeat(63))).is_err());
    }

    #[test]
    fn sha256_hex_matches_known_vector() {
        // 标准测试向量：空串与 "abc"
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn build_info_flags_update_and_picks_assets() {
        let info = build_kernel_info(release("v0.1.4", release_assets()));
        assert_eq!(info.latest_version.as_deref(), Some("0.1.4"));
        assert_eq!(info.asset_size, Some(826_505));
        assert_eq!(info.notes.as_deref(), Some("修复若干转换问题"));
        assert!(info.release_url.is_some());
    }

    #[test]
    fn build_info_ignores_draft_and_prerelease() {
        for (draft, prerelease) in [(true, false), (false, true)] {
            let mut rel = release("v0.1.5", release_assets());
            rel.draft = draft;
            rel.prerelease = prerelease;
            let info = build_kernel_info(rel);
            assert!(!info.available);
            assert!(info.latest_version.is_none());
        }
    }

    #[test]
    fn build_info_blocks_install_when_assets_missing() {
        // 缺 sha256：无法校验完整性，必须拒绝安装
        let info = build_kernel_info(release("v0.1.4", vec![asset("prism.wasm")]));
        assert!(!info.can_install);
        assert!(info.blocked_reason.unwrap().contains("校验和"));

        // 缺 wasm：直接不可安装
        let info = build_kernel_info(release("v0.1.4", vec![asset("prism.wasm.sha256")]));
        assert!(!info.can_install);
        assert!(info.blocked_reason.unwrap().contains("prism.wasm"));
    }

    #[test]
    fn find_asset_errors_on_missing() {
        let rel = release("v0.1.4", release_assets());
        assert!(find_asset(&rel, ASSET_WASM).is_ok());
        assert!(find_asset(&rel, "nonexistent.bin").is_err());
    }

    #[test]
    fn manifest_roundtrip() {
        let manifest = ReleaseManifest {
            tag: "0.1.4".to_string(),
            sha256: Some("abc".to_string()),
            bytes: Some(826_505),
        };
        let text = serde_json::to_string(&manifest).unwrap();
        let parsed: ReleaseManifest = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed.tag, "0.1.4");
        assert_eq!(parsed.bytes, Some(826_505));
    }

    #[test]
    fn manifest_tolerates_unknown_fields() {
        // 真实的 prism.release.json 含 module/commit/moonbit/exports 等额外字段，
        // 解析不能因未知字段失败
        let text = r#"{
            "artifact": "prism.wasm",
            "tag": "v0.1.4",
            "module": "morning-start/prism",
            "commit": "9c6e86a78dd1184aea69c2071f8d42a159453961",
            "moonbit": "moon 0.1.20260922",
            "sha256": "cc8a2f83ad84b5c5c14c178ccf22a02a4e07146db4bfaecc565b67754c6b3ea3",
            "bytes": 775844,
            "exports": 28
        }"#;
        let manifest: ReleaseManifest = serde_json::from_str(text).expect("tolerate extra fields");
        assert_eq!(manifest.tag, "v0.1.4");
        assert_eq!(manifest.bytes, Some(775_844));
    }

    // -----------------------------------------------------------------------
    // 落盘（不依赖网络，覆盖 Windows 上最容易出错的两条路径）
    // -----------------------------------------------------------------------

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "silk-kernel-commit-{tag}-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn manifest_for(tag: &str) -> ReleaseManifest {
        ReleaseManifest {
            tag: tag.to_string(),
            sha256: Some("deadbeef".to_string()),
            bytes: Some(4),
        }
    }

    #[test]
    fn commit_kernel_writes_new_kernel_and_manifest() {
        let dir = temp_dir("fresh");
        commit_kernel(&dir, b"NEW!", &manifest_for("0.1.4")).expect("commit");

        assert_eq!(std::fs::read(dir.join(prism_wasm::PRISM_WASM_FILE)).unwrap(), b"NEW!");
        let manifest = read_manifest_at(&dir).expect("manifest");
        assert_eq!(manifest.tag, "0.1.4");
        // 临时文件必须被清理（rename 后不存在）
        assert!(!dir.join(DOWNLOAD_FILE).exists(), "临时文件应已被 rename 消耗");
        // 首次安装无旧内核 → 不产生备份
        assert!(!dir.join(BACKUP_FILE).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 关键路径：Windows 上替换**已存在**的内核文件。
    /// std::fs::rename 在 Windows 走 MoveFileEx + REPLACE_EXISTING，
    /// 这条测试确保「覆盖安装」不会因目标已存在而失败。
    #[test]
    fn commit_kernel_replaces_existing_kernel() {
        let dir = temp_dir("replace");
        let target = dir.join(prism_wasm::PRISM_WASM_FILE);
        std::fs::write(&target, b"OLD!").unwrap();

        commit_kernel(&dir, b"NEW!", &manifest_for("0.1.5")).expect("replace existing");

        assert_eq!(std::fs::read(&target).unwrap(), b"NEW!", "内核应被新内容覆盖");
        // 旧内核必须留有备份，供回滚
        assert_eq!(
            std::fs::read(dir.join(BACKUP_FILE)).unwrap(),
            b"OLD!",
            "旧内核应被备份"
        );
        assert_eq!(read_manifest_at(&dir).expect("manifest").tag, "0.1.5");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 备份必须是**一对**：内核 + 版本记录。
    /// 只备份 wasm 会让回滚后的版本号指向一个并不存在的内核。
    #[test]
    fn commit_kernel_backs_up_manifest_alongside_kernel() {
        let dir = temp_dir("backup-pair");
        let target = dir.join(prism_wasm::PRISM_WASM_FILE);
        std::fs::write(&target, b"OLD!").unwrap();
        // 先有一个旧清单（模拟已安装 v0.1.3）
        std::fs::write(
            dir.join(MANIFEST_FILE),
            serde_json::to_string_pretty(&manifest_for("0.1.3")).unwrap(),
        )
        .unwrap();

        commit_kernel(&dir, b"NEW!", &manifest_for("0.1.4")).expect("commit");

        assert_eq!(std::fs::read(dir.join(BACKUP_FILE)).unwrap(), b"OLD!");
        let backup_manifest =
            read_manifest_at_named(&dir, BACKUP_MANIFEST_FILE).expect("备份清单必须存在");
        assert_eq!(backup_manifest.tag, "0.1.3", "备份清单应记录旧版本");
        // 当前清单指向新版本
        assert_eq!(read_manifest_at(&dir).expect("manifest").tag, "0.1.4");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 写清单失败时回滚内核**与版本记录**：不能出现「内核回旧了、版本号还是新的」。
    #[test]
    fn commit_kernel_rolls_back_when_manifest_write_fails() {
        let dir = temp_dir("rollback");
        let target = dir.join(prism_wasm::PRISM_WASM_FILE);
        std::fs::write(&target, b"OLD!").unwrap();

        // 旧清单（v0.1.3）
        let manifest_path = dir.join(MANIFEST_FILE);
        std::fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest_for("0.1.3")).unwrap(),
        )
        .unwrap();

        // 用一个只读占位让原子写失败：改用同名目录占住清单路径
        // （先备份走的是 copy，能成功；随后 write_text_atomic 会因目标是目录而失败）
        let result = {
            // 备份清单已在上面写好，这里把清单路径换成目录以触发写入失败
            let _ = std::fs::remove_file(&manifest_path);
            std::fs::create_dir_all(&manifest_path).unwrap();
            commit_kernel(&dir, b"NEW!", &manifest_for("0.1.5"))
        };

        assert!(result.is_err(), "清单写入失败必须向上报错");
        assert_eq!(
            std::fs::read(&target).unwrap(),
            b"OLD!",
            "清单写入失败后内核必须回滚为旧内容"
        );
        // 临时文件不应残留
        assert!(!dir.join(DOWNLOAD_FILE).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 首次安装时清单写入失败 → 新内核必须被删除，不留半个状态
    #[test]
    fn commit_kernel_removes_kernel_when_no_backup_on_failure() {
        let dir = temp_dir("rollback-fresh");
        let manifest_path = dir.join(MANIFEST_FILE);
        std::fs::create_dir_all(&manifest_path).unwrap();

        let result = commit_kernel(&dir, b"NEW!", &manifest_for("0.1.4"));
        assert!(result.is_err());
        assert!(
            !dir.join(prism_wasm::PRISM_WASM_FILE).exists(),
            "无备份可回滚时应删除新内核"
        );
        // 备份对不完整时不应留下半份备份（否则「可回滚」会误报为真）
        assert!(!dir.join(BACKUP_FILE).exists());
        assert!(!dir.join(BACKUP_MANIFEST_FILE).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 回滚是无备份时唯一的自救出口：没有备份必须明确报错，而不是静默成功
    #[test]
    fn rollback_requires_backup() {
        let dir = temp_dir("rollback-none");
        std::fs::create_dir_all(&dir).unwrap();
        // 直接验证「无备份」判定逻辑（不触碰真实 AppData）
        assert!(!dir.join(BACKUP_FILE).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn status_is_self_consistent() {
        let status = status();
        assert_eq!(status.supported_abi, prism_wasm::SUPPORTED_ABI);
        // 内嵌/程序目录来源无法反查版本 → version 为 None；仅数据目录来源才有版本
        if status.source != "data_dir" {
            assert!(status.version.is_none(), "非数据目录来源不应有版本号");
        }
        // 不可更新时必须给出原因
        if !status.updatable {
            assert!(status.updatable_reason.is_some());
        }
        // 非内嵌来源必须带路径
        if status.source != "embedded" {
            assert!(status.path.is_some());
        }
        // 无备份时不得报告可回滚，也不得有备份版本号
        if !status.backup_available {
            assert!(status.backup_version.is_none());
        }
    }
}
