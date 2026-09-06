#!/usr/bin/env bun
/**
 * 一键同步版本号脚本（Bun 直接运行）
 *
 * 用法:
 *   bun scripts/bump-version.ts 0.6.1      # 或 bun run bump:version 0.6.1
 *   bun scripts/bump-version.ts v0.6.1     # 自动去掉 v 前缀
 *
 * 同步范围:
 *   - package.json                "version": "..."
 *   - src-tauri/tauri.conf.json   "version": "..."
 *   - src-tauri/Cargo.toml        version = "..."
 *   - src-tauri/Cargo.lock        [[package]] name = "silk" 条目（第三方依赖不动）
 *   - README.md                   shields.io version 徽章
 */

import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");

// ---------- 参数解析 ----------
const arg = process.argv.slice(2).find((a) => !a.startsWith("-"));
if (!arg) {
  console.error("用法: bun scripts/bump-version.ts <版本号>   例如: bun scripts/bump-version.ts 0.6.1");
  process.exit(1);
}
const next = arg.startsWith("v") ? arg.slice(1) : arg;
if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(next)) {
  console.error(`无效版本号: ${arg}（期望类似 0.6.1 或 v0.6.1）`);
  process.exit(1);
}

// ---------- 工具 ----------
function read(path: string): string {
  return readFileSync(resolve(root, path), "utf8");
}
function write(path: string, content: string): void {
  writeFileSync(resolve(root, path), content, "utf8");
}

// 整体替换（from -> to 均为完整上下文），要求至少命中一次
function replaceAll(content: string, from: string, to: string, label: string): string {
  if (!content.includes(from)) {
    throw new Error(`${label}: 未找到 "${from}"，请检查文件格式是否变化`);
  }
  return content.split(from).join(to);
}

// ---------- 读取当前版本（以 package.json 为准）----------
const pkgRaw = read("package.json");
const old = /"version"\s*:\s*"([^"]+)"/.exec(pkgRaw)?.[1];
if (!old) {
  console.error("package.json: 未找到 version 字段");
  process.exit(1);
}
if (old === next) {
  console.log(`版本已经是 ${next}，无需修改。`);
  process.exit(0);
}

// ---------- 内存中替换全部文件（全部成功后一次性写入）----------
const changes: Array<{ label: string }> = [];
function plan(path: string, from: string, to: string, label: string): string {
  const content = read(path);
  const updated = replaceAll(content, from, to, label);
  if (updated !== content) changes.push({ label });
  return updated;
}

const outputs = new Map<string, string>();
outputs.set("package.json", plan("package.json", `"version": "${old}"`, `"version": "${next}"`, "package.json"));
outputs.set(
  "src-tauri/tauri.conf.json",
  plan("src-tauri/tauri.conf.json", `"version": "${old}"`, `"version": "${next}"`, "src-tauri/tauri.conf.json"),
);
outputs.set(
  "src-tauri/Cargo.toml",
  plan("src-tauri/Cargo.toml", `version = "${old}"`, `version = "${next}"`, "src-tauri/Cargo.toml"),
);
// Cargo.lock：只改 name = "silk" 的 package 块，带块首行上下文保证唯一
outputs.set(
  "src-tauri/Cargo.lock",
  plan(
    "src-tauri/Cargo.lock",
    `name = "silk"\nversion = "${old}"`,
    `name = "silk"\nversion = "${next}"`,
    "src-tauri/Cargo.lock (silk 条目)",
  ),
);
outputs.set(
  "README.md",
  plan("README.md", `version-${old}-blue`, `version-${next}-blue`, "README.md (version 徽章)"),
);

for (const [path, content] of outputs) {
  write(path, content);
}

// ---------- 汇总 ----------
console.log(`版本号 ${old} -> ${next} 已同步：`);
for (const c of changes) {
  console.log(`  ✓ ${c.label}`);
}

// ---------- 残留检查（字面匹配，排除构建/工作区目录）----------
const { execSync } = await import("node:child_process");
const excludes = [
  "--exclude-dir=node_modules",
  "--exclude-dir=target",
  "--exclude-dir=.git",
  "--exclude-dir=.agent-workplace",
  "--exclude-dir=.workbuddy",
].join(" ");
try {
  const out = execSync(`grep -rnF ${excludes} "${old}" --include="*.json" --include="*.toml" --include="*.md" .`, {
    cwd: root,
    encoding: "utf8",
  });
  const hits = out.split("\n").filter(Boolean);
  console.warn("⚠  以下位置仍残留旧版本号，请人工确认：");
  for (const h of hits) console.warn("    " + h);
} catch {
  console.log("  ✓ 无残留旧版本号");
}
