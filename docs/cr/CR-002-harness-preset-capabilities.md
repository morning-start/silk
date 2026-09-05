# CR-002：预设模块 Harness 配置能力扩展

- 状态：已批准并排期
- 提出人：用户（对话 IM，2026-09-04）
- 分级：中度
- 排期：iteration-005
- 执行方略：spec

## 原始需求

> 把 E:\Workplace\APP\Tauri\cc-switch 中的快速切换harness的配置的能力抄过来，当前项目中是预设模块。

## 已确认范围

在 Silk 的预设模块中迁移 `cc-switch` 已验证的 harness 配置能力：结构化字段、模型选择、原生高级配置及其到外部 CLI live 配置的正确投影。

仅迁移能力，不迁移供应商预设清单、系统托盘、OAuth/多账号、Universal Provider、代理接管 UI。

## 影响评估

- 数据库：不变。`presets.settings_config` 继续是配置快照的唯一持久化来源。
- 前端：扩展 `src/config/harnessForms.ts` 的字段类型和五个 harness 的编解码；`PresetsView.vue` 渲染新字段与校验错误。
- 后端：补齐五个 `HarnessWriter` 对各自原生配置结构的投影；继续使用现有原子写、快照回滚、旧配置剥离和 live 回填。
- 风险：配置字段写到错误层级会导致 CLI 忽略配置或拒绝加载。每个 writer 必须以格式级测试验证写入结果。

## 验收边界

1. 五个现有 harness 的高级字段能在预设表单中编辑、保存、再次打开并完整回显。
2. 激活预设后，字段被投影到各自外部 CLI 的原生 JSON、TOML 或 YAML 位置，不覆盖预设不拥有的用户配置。
3. 不支持的 OAuth、代理接管、通用供应商和供应商目录均不出现在本次变更中。
4. 错误配置不会写入 live 文件；写入失败恢复原有文件和激活状态。
