# Implementation Plan: CLI `--url` + Self-Signed Cert Tolerance + Ubuntu 24.04 Build

**Branch**: `001-cli-url-ssl-ubuntu` | **Date**: 2026-04-30 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/001-cli-url-ssl-ubuntu/spec.md`

## Summary

为已打包的 Pake 二进制新增**运行时 `--url` 覆盖**与**显式 SSL 校验绕过**两项能力，并在打包链路新增**Ubuntu 24.04 专属、单一 `.deb` 产物**的构建路径。技术路线：Rust 端在 `main.rs` 解析 `--url` / `--ignore-cert` → 通过环境变量传给 `lib.rs::run` → 在 `util.rs::get_pake_config` 注入到内存 `PakeConfig`；SSL 复用 Pake 已有的 `WindowConfig.ignore_certificate_errors` 字段（`window.rs:292` 已实现 Chromium `--ignore-certificate-errors` 注入）；Ubuntu 24.04 构建链路在 `bin/builders/LinuxBuilder.ts` 与 `tauri.linux.conf.json` 锁定 `targets=["deb"]`、`active=true`、`depends=["libwebkit2gtk-4.1-0", ...]`，并新增 `pnpm run build:ubuntu24` 与对应 `.github/workflows/build-ubuntu24.yml`。

## Technical Context

**Language/Version**: Rust 1.85.0（[src-tauri/Cargo.toml](../../src-tauri/Cargo.toml) `rust-version`）；TypeScript 5.x + Node 20+（[bin/cli.ts](../../bin/cli.ts)，commander）
**Primary Dependencies**: Tauri 2.10.2（`tray-icon`/`macos-proxy`）、tauri-plugin-* 全家桶、commander 11.x、tauri-build 2.5.5；**禁止**为本特性引入新 Rust crate（如 `clap`），仅用 `std::env::args` 解析（与现状一致）
**Storage**: 无新增；`pake.json` 在内存中可变覆盖，不写盘
**Testing**: Rust：`cargo check` + 新增 `#[test]` 单元测试覆盖 `parse_runtime_args` 与 `apply_runtime_overrides`；TS：vitest（[tests/unit/](../../tests/unit/)） 新增 LinuxBuilder Ubuntu 24.04 路径测试；端到端：在 Ubuntu 24.04 GHA runner 内构建 → 安装 deb → 启动 → 截屏断言
**Target Platform**: 运行时三端通用（macOS / Windows / Linux）；构建产物本特性仅交付 Ubuntu 24.04 amd64 deb；现有 mac/win/其他 linux 构建路径保持原样
**Project Type**: desktop-app（Tauri 双层：TS CLI 编排 + Rust 应用运行时 + JS 注入层）
**Performance Goals**: 启动新增开销 ≤ 5ms（仅多一次 args 扫描与一次 env_var 读取）；二进制体积偏差 ≤ ±5%；deb 安装包大小偏差 ≤ ±5%
**Constraints**: 守 [pake.speckit.constitution](../../pake.speckit.constitution) §13（体积/性能）、§10（配置层不直改用户 `tauri.conf.json`，CLI 输出走 `bin/options/logger.ts`）、§12（跨平台用 `#[cfg]`）、§15（最小权限，不开 shell 通配）
**Scale/Scope**: 改动量 ≤ 8 个文件；新增 ≤ 250 行；不引入 DB/网络新依赖

## Constitution Check

参照项目宪章 [pake.speckit.constitution](../../pake.speckit.constitution)：

| Gate | 检查 | 结论 |
|---|---|---|
| §0 体积/启动 | `--url` 解析在 `main.rs` 已用 `std::env::args`，无新依赖；deb 仅减不加 | ✅ Pass |
| §6 错误处理 | `--url` 非法 URL → 进程退出 + stderr；TS 端 builder 错误透传退出码 | ✅ Pass |
| §7 并发 | 无新异步路径 | ✅ Pass |
| §10 Tauri 模式 | 配置覆盖在 `util.rs` 集中处理，**不**直接改用户 `tauri.conf.json` 文件 | ✅ Pass |
| §12 跨平台 | SSL/构建均用 `#[cfg]`/平台 builder 隔离 | ✅ Pass |
| §13 性能/体积 | 不改 `[profile.release]`；不引入新 Rust crate | ✅ Pass |
| §14 反模式 | 不在 invoke 里 `.unwrap()`；无重型依赖；不污染 window | ✅ Pass |
| §15 安全 | SSL 绕过必须**显式**（CLI 开关 / `pake.json` 字段），废除 localhost 隐式启用 | ✅ Pass |
| §16 测试 | 为 args 解析与 LinuxBuilder ubuntu24 路径补单测 | ✅ Pass |

**Post-Design Re-check**：见 Phase 1 末尾。

## Project Structure

### Documentation (this feature)

```text
specs/001-cli-url-ssl-ubuntu/
├── plan.md              # 本文件
├── spec.md              # 功能规格
├── research.md          # Phase 0：决策与备选
├── data-model.md        # Phase 1：内存数据模型
├── quickstart.md        # Phase 1：构建/验证步骤
├── contracts/
│   └── cli.md           # Phase 1：CLI 参数契约
└── tasks.md             # /speckit.tasks 生成（不在本步骤）
```

### Source Code (repository root)

```text
bin/
├── cli.ts                            # 不变
├── helpers/cli-program.ts            # 不变（运行时 --url 由二进制自身解析；TS 端不参与）
├── builders/
│   ├── BaseBuilder.ts                # 不变
│   └── LinuxBuilder.ts               # ✏ 新增 ubuntu24 模式：锁定 targets=['deb']、注入 deb depends
└── helpers/tauriConfig.ts            # ✏ ubuntu24 模式时合并 linux deb depends

src-tauri/
├── Cargo.toml                        # 🛠 修复：恢复 tokio/tauri 换行；移除无关 chrono；移除无关 F4 改动
├── src/
│   ├── main.rs                       # ✏ 扩展 args 解析：支持 --ignore-cert；废除 localhost 隐式
│   ├── lib.rs                        # 🛠 修复：恢复 .on_window_event(...) 起始行；剥离 F4 perf 监控
│   ├── util.rs                       # ✏ 仅保留 PAKE_RUNTIME_URL 覆盖；删除 localhost 自动 ignore；新增 PAKE_IGNORE_CERT 读取
│   └── app/invoke.rs                 # 🛠 移除 F4 get_perf_stats（拆到独立 spec）
├── tauri.linux.conf.json             # ✏ ubuntu24 模式恢复 active=true、targets=['deb']、deb depends
└── pake.json                         # 不变（既有 ignore_certificate_errors 字段沿用）

package.json                          # ✏ 新增 scripts: build:ubuntu24
.github/workflows/
└── build-ubuntu24.yml                # 🆕 ubuntu-24.04 runner，仅一个 job，产出单一 deb

tests/unit/
└── linux-builder-ubuntu24.test.ts    # 🆕 vitest

src-tauri/src/
└── (Rust 单元测试就近放在被测模块 #[cfg(test)] 子模块)
```

**Structure Decision**: 采用 Pake 既有的"TS CLI（[bin/](../../bin/)）+ Rust 运行时（[src-tauri/src/](../../src-tauri/src/)）+ JSON 配置（[src-tauri/*.conf.json](../../src-tauri/)）"三层结构。本特性所有改动落在这三层既有目录内，**不**新增顶级目录、**不**新增子项目。运行时能力（`--url`/`--ignore-cert`）落在 Rust 端（直接对最终二进制生效），构建路径在 TS Builder + tauri 配置 + GHA 工作流；JS 注入层不动。

## Complexity Tracking

> 本表仅在 Constitution Check 有违反需说明时填写。

当前所有 Gate 均 Pass，**无需填写**。

---

## Phase 0: Outline & Research

详见 [research.md](./research.md)。已解决的关键决策：

1. **运行时 args 解析**：沿用 `std::env::args` 手解析，**不**引入 `clap`（守 §13 体积约束）
2. **运行时 URL 注入路径**：`main.rs → env::set_var(PAKE_RUNTIME_URL) → util.rs::get_pake_config 读取并改 PakeConfig`，**不**走 invoke（启动期需要）
3. **SSL 绕过机制**：复用既有 `WindowConfig.ignore_certificate_errors` + [src-tauri/src/app/window.rs:292](../../src-tauri/src/app/window.rs) 已实现的 Chromium `--ignore-certificate-errors` 注入；**不**自实现证书校验回调
4. **localhost 隐式启用废除**：原型中 `host=="127.0.0.1"||"localhost"` 自动启用违反 §15 显式安全原则，必须移除
5. **Ubuntu 24.04 专属构建模式**：通过新 npm script + 现有 LinuxBuilder 注入 `targets='deb'` + tauri 配置层叠 + 独立 GHA workflow，**不**删除现有跨平台代码
6. **F4 perf 监控剥离**：与本特性正交，移到 `002-linux-perf-monitor`（独立 spec）

**Output**: research.md（无遗留 NEEDS CLARIFICATION）

## Phase 1: Design & Contracts

**Prerequisites**: research.md 完成

1. **数据模型** → [data-model.md](./data-model.md)
   - `PakeConfig.windows[0].url` / `url_type`：运行时可覆盖
   - `WindowConfig.ignore_certificate_errors`：三态合并语义
   - `LinuxBuildOptions.ubuntu24Only`：新增 TS 内部状态

2. **CLI 接口契约** → [contracts/cli.md](./contracts/cli.md)
   - 二进制运行时参数：`--url <URL>` / `--ignore-cert`
   - `pnpm` 构建脚本：`build:ubuntu24`
   - GHA workflow 输入/产物名

3. **Quickstart** → [quickstart.md](./quickstart.md)
   - 从干净仓库到 Ubuntu 24.04 deb 的端到端步骤
   - 三个 user story 的手动验证脚本

4. **Agent context 更新**：在 [.github/copilot-instructions.md](../../.github/copilot-instructions.md) `<!-- SPECKIT START/END -->` 之间插入到本 plan 的引用

**Post-Design Constitution Re-check**：所有 Gate 仍 Pass，无新违反。

**Output**: data-model.md、contracts/cli.md、quickstart.md、更新后的 copilot-instructions.md

## Key rules

- 文件系统操作用绝对路径；文档与 agent context 中的引用用项目相对路径
- Constitution 任何 Gate 失败 / NEEDS CLARIFICATION 未解 → ERROR
- 本 plan **到 Phase 1 即停**；任务拆分由 `/speckit.tasks` 完成
