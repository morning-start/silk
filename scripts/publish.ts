#!/usr/bin/env bun
/**
 * 一键发布脚本（Bun 直接运行，Windows cmd shell 兼容）
 *
 * 用法:
 *   bun scripts/publish.ts 0.9.3                  # bump 版本 + 构建 + 签名 + latest.json + gh release + push
 *   bun scripts/publish.ts 0.9.3 --skip-build     # 跳过构建（复用已有产物）
 *   bun scripts/publish.ts 0.9.3 --no-commit      # 不自动 git commit
 *   bun scripts/publish.ts 0.9.3 --no-push        # 不自动 push（只创建 GitHub Release）
 *   bun scripts/publish.ts 0.9.3 --notes "修复 xxx"
 *
 * 流程: bump-version → git commit → tauri build → 收集安装包 + 签名(.sig)
 *       → 生成 latest.json → gh release create 上传资产
 *
 * 前置要求:
 *   - 签名私钥位于 ~/.tauri/silk.key（tauri signer generate -w ~/.tauri/silk.key --ci 生成）
 *   - 已安装 GitHub CLI 并登录（gh auth login）
 */

import { execSync } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { basename, join, resolve } from "node:path";
import { homedir, tmpdir } from "node:os";

const root = resolve(import.meta.dirname, "..");
const REPO = "morning-start/silk";
const CLI = join(root, "node_modules", "@tauri-apps", "cli", "tauri.js");
const KEY_PATH = join(homedir(), ".tauri", "silk.key");
const BUNDLE_DIR = resolve(root, "src-tauri", "target", "release", "bundle");

// ---------- 参数解析 ----------
const flags = new Set<string>();
let version = "";
let notesArg = "";
for (const a of process.argv.slice(2)) {
  if (a === "--notes") continue;
  if (a.startsWith("--notes=")) {
    notesArg = a.slice("--notes=".length);
  } else if (a.startsWith("-")) {
    flags.add(a);
  } else {
    version = a;
  }
}
// --notes "xxx" 形式：取下一个参数
const idx = process.argv.indexOf("--notes");
if (idx !== -1 && process.argv[idx + 1]) {
  notesArg = process.argv[idx + 1];
}

if (!version) {
  console.error("用法: bun scripts/publish.ts <版本号> [--skip-build] [--no-commit] [--no-push] [--notes <文本>]");
  process.exit(1);
}
version = version.replace(/^v/, "");
if (!/^\d+\.\d+\.\d+/.test(version)) {
  console.error(`无效版本号: ${version}（期望类似 0.9.3）`);
  process.exit(1);
}

// ---------- 工具 ----------
function run(cmd: string) {
  console.log(`\n> ${cmd}`);
  execSync(cmd, { cwd: root, stdio: "inherit", shell: process.env.ComSpec });
}
function capture(cmd: string): string {
  return execSync(cmd, { cwd: root, encoding: "utf8", shell: process.env.ComSpec })
    .trim()
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter(Boolean)
    .join("\n");
}
function quote(p: string): string {
  return `"${p.replace(/"/g, '\\"')}"`;
}

// ---------- 前置检查 ----------
console.log(`\n=== Silk 发布流程 v${version} ===`);
if (!existsSync(KEY_PATH)) {
  console.error(`找不到签名私钥 ${KEY_PATH}`);
  console.error("请先运行: node node_modules/@tauri-apps/cli/tauri.js signer generate -w <你的路径> --ci");
  process.exit(1);
}
try {
  capture("gh --version");
} catch {
  console.error("未安装 GitHub CLI (gh)。请安装并登录: https://cli.github.com");
  process.exit(1);
}

// ---------- 1. 同步版本号 ----------
run(`bun scripts/bump-version.ts ${version}`);

// ---------- 2. git commit ----------
if (!flags.has("--no-commit")) {
  run(`git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock README.md`);
  run(`git commit -m "chore(release): bump to v${version}"`);
}

// ---------- 3. 构建 ----------
if (!flags.has("--skip-build")) {
  run(`node ${quote(CLI)} build`);
}

// ---------- 4. 收集安装包并签名 ----------
interface Artifact {
  file: string;
  platform: string;
}

/** 从安装包文件名推断 Tauri updater 平台标识 */
function detectPlatform(fileName: string): string | null {
  const f = fileName.toLowerCase();
  let arch: string | null = null;
  if (f.includes("aarch64") || f.includes("arm64")) arch = "aarch64";
  else if (f.includes("x64") || f.includes("x86_64") || f.includes("amd64")) arch = "x86_64";
  else if (f.includes("x86") || f.includes("i686")) arch = "i686";
  if (!arch) return null;

  if (f.endsWith(".exe") && (f.includes("setup") || f.includes("installer"))) {
    if (arch === "aarch64") return "windows-aarch64";
    if (arch === "x86_64") return "windows-x86_64";
    return "windows-i686";
  }
  if (f.endsWith(".dmg")) return `darwin-${arch}`;
  if (f.endsWith(".app.tar.gz")) return `darwin-${arch}`;
  if (f.endsWith(".deb") || f.endsWith(".appimage") || f.endsWith(".rpm")) return `linux-${arch}`;
  return null;
}

function collectArtifacts(dir: string, out: Artifact[] = []): Artifact[] {
  if (!existsSync(dir)) return out;
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    const st = statSync(full);
    if (st.isDirectory()) {
      collectArtifacts(full, out);
    } else if (st.isFile() && !entry.endsWith(".sig") && !entry.endsWith(".blockmap")) {
      const platform = detectPlatform(entry);
      if (platform) out.push({ file: full, platform });
    }
  }
  return out;
}

const artifacts = collectArtifacts(BUNDLE_DIR);
if (artifacts.length === 0) {
  console.error(`在 ${BUNDLE_DIR} 下没有找到可发布的安装包（NSIS exe / dmg / deb / appimage）。`);
  process.exit(1);
}
console.log(`\n找到 ${artifacts.length} 个安装包:`);
for (const a of artifacts) console.log(`  [${a.platform}] ${basename(a.file)}`);

const platforms: Record<string, { signature: string; url: string }> = {};
const uploads: string[] = [];

for (const a of artifacts) {
  const sigPath = `${a.file}.sig`;
  if (!existsSync(sigPath)) {
    console.log(`\n签名: ${basename(a.file)}`);
    run(`node ${quote(CLI)} signer sign ${quote(a.file)} -f ${quote(KEY_PATH)}`);
  }
  if (!existsSync(sigPath)) {
    console.error(`签名文件缺失: ${sigPath}`);
    process.exit(1);
  }
  const signature = readFileSync(sigPath, "utf8").trim();
  const url = `https://github.com/${REPO}/releases/download/v${version}/${encodeURIComponent(basename(a.file))}`;
  platforms[a.platform] = { signature, url };
  uploads.push(a.file);
}

// ---------- 5. 生成 latest.json ----------
function getReleaseNotes(): string {
  try {
    const tags = capture("git tag --sort=-version:refname");
    const prevTag = tags.split("\n")[0];
    if (!prevTag) return "新版本发布";
    const log = capture(`git log --oneline ${prevTag}..HEAD`);
    return log || "新版本发布";
  } catch {
    return "新版本发布";
  }
}
const notes = notesArg || getReleaseNotes().replace(/"/g, "'");

const latestJson = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms,
};
const tmpDir = mkdtempSync(join(tmpdir(), "silk-release-"));
const latestPath = join(tmpDir, "latest.json");
writeFileSync(latestPath, JSON.stringify(latestJson, null, 2));
console.log(`\nlatest.json 已生成: ${latestPath}`);
console.log(JSON.stringify(latestJson, null, 2));

// ---------- 6. 创建 GitHub Release 并上传 ----------
const uploadArgs = [...uploads.map(quote), quote(latestPath)].join(" ");
run(`gh release create v${version} ${uploadArgs} --repo ${REPO} --title "v${version}" --notes ${quote(notes)}`);

// ---------- 7. push（可选）----------
if (!flags.has("--no-push")) {
  run("git push");
  run("git push --tags");
}

console.log(`\n=== 发布完成: v${version} ===`);
// 两条端点分工不同，别混：
// - REST API：应用内「检查更新」走它（只需网络可达，不依赖签名产物）
// - 插件清单：tauri-plugin-updater 静默安装走它（要求 latest.json + .sig 都已上传）
console.log(`更新检查（REST API）  : https://api.github.com/repos/${REPO}/releases/latest`);
console.log(`静默安装清单（插件）  : https://github.com/${REPO}/releases/latest/download/latest.json`);
