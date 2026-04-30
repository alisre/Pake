---
description: "Tasks for 001-cli-url-ssl-ubuntu"
---

# Tasks: CLI `--url` 运行时覆盖 + 自定义证书 SSL 容忍 + Ubuntu 24.04 专属构建

**Input**: Design documents from `/specs/001-cli-url-ssl-ubuntu/`
**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/cli.md](./contracts/cli.md), [quickstart.md](./quickstart.md)

**Tests**: 包含针对 Rust args 解析与 LinuxBuilder ubuntu24 路径的单元测试（spec FR-009/FR-010 修复 + 三大用户故事均需可独立验证）。端到端验证由 [quickstart.md](./quickstart.md) 覆盖，不在 tasks 中重复。

**Organization**: 按 User Story 分组，每个故事可独立实现并验证。

## Format: `[ID] [P?] [Story] Description`

- **[P]**: 可并行（不同文件、无未完成依赖）
- **[Story]**: 所属用户故事（[US1]/[US2]/[US3]），Setup/Foundational/Polish 不带 Story 标签

## Path Conventions

- Pake 项目布局（desktop-app）：
  - Rust 运行时：[src-tauri/src/](../../src-tauri/src/)
  - TS CLI：[bin/](../../bin/)
  - Tauri 配置：[src-tauri/*.conf.json](../../src-tauri/)
  - 测试：Rust `#[cfg(test)]` 就近放被测模块；TS 放 [tests/unit/](../../tests/unit/)
  - 工作流：[.github/workflows/](../../.github/workflows/)

---

## Phase 1: Setup（共享准备）

**Purpose**: 把当前分支拉回到一个干净、可编译的基线，剥离与本特性无关的原型噪声。

- [x] T001 在仓库根读取并确认当前分支为 `001-cli-url-ssl-ubuntu`，记录 baseline commit SHA 与基线 deb 大小（若 main 有最近 release artifact 则 `ls -la` 记录；用于 SC-005 后续比对）；运行 `pnpm install` 与 `cd src-tauri && cargo check` 复现现有缺陷（**预期**：`cargo check` 报错 — 这是 FR-009 待修目标，红状态属正常基线）
- [x] T002 [P] 不创建独立 specs 目录，仅在本 spec [research.md](./research.md) R-005 行末追记一行：「F4 perf 监控代码片段保留于 baseline commit `<SHA>`，未来独立 spec 通过 `git show <SHA>:src-tauri/src/lib.rs` 恢复」

---

## Phase 2: Foundational（阻塞前置）

**Purpose**: 修复阻断编译的原型缺陷 + 剥离 F4 噪声 + 移除 §15 违规的 localhost 隐式启用。三大用户故事均依赖此阶段完成。

**⚠️ CRITICAL**: 本阶段未完成前，任何用户故事任务都无法跑通。

- [x] T004 修复 [src-tauri/Cargo.toml](../../src-tauri/Cargo.toml) 依赖压行问题：把 `tokio = {...}chrono = {...}tauri = {...}` 拆回三行；**移除 `chrono` 依赖**（与 F4 一同剥离）
- [x] T005 剥离 F4 perf 监控：在 [src-tauri/src/lib.rs](../../src-tauri/src/lib.rs) 删除 `#[cfg(target_os = "linux")] use chrono;`、删除 setup 闭包内 60 秒采样 spawn 块、删除 `get_perf_stats` 注册
- [x] T006 修复 [src-tauri/src/lib.rs](../../src-tauri/src/lib.rs) 中被破坏的 `.on_window_event(move |_window, _event| {` 起始行（恢复完整闭包，使 `if let WindowEvent::CloseRequested` 块语法正确）
- [x] T007 剥离 F4 perf 监控：在 [src-tauri/src/app/invoke.rs](../../src-tauri/src/app/invoke.rs) 删除 `get_perf_stats` 函数与 `#[cfg(target_os = "linux")] use chrono;`
- [x] T008 在 [src-tauri/src/util.rs](../../src-tauri/src/util.rs) 中**移除** `host=="127.0.0.1" || host=="localhost"` 自动启用 `ignore_certificate_errors` 的代码块（违反宪章 §15 显式安全原则；FR-005）；`use ... Url` 若失去使用方需一并清理
- [ ] T009 运行 `cd src-tauri && cargo check` 验证可编译；运行 `pnpm test` 验证既有 TS 测试不受影响（baseline 绿）

**Checkpoint**: 仓库回到可编译、可运行的干净基线。可以并行进入三个用户故事。

---

## Phase 3: User Story 1 — `--url` 运行时覆盖（Priority: P1）🎯 MVP

**Goal**: 已打包二进制接受 `--url <URL>`，在加载窗口前覆盖 `pake.json` 中的编译期 URL，本次进程内有效，不写盘。

**Independent Test**: 构建 + 安装后，`./browser --url https://stage.intra.example.com` 加载该 URL；`./browser` 回到默认 URL；`./browser --url ftp://x` 退出码 1 + 明确报错。

### Tests for User Story 1

- [x] T010 [P] [US1] 在 [src-tauri/src/main.rs](../../src-tauri/src/main.rs) 顶部新增 `#[cfg(test)] mod tests` 子模块，添加 `parse_runtime_args_url_*` 三个测试：合法 https、合法 http、缺失值、非 http/https 协议；使用纯函数化的 `parse_runtime_args(args: impl IntoIterator<Item=String>) -> Result<RuntimeOverrides, String>` 以便可测
- [x] T011 [P] [US1] 在 [src-tauri/src/util.rs](../../src-tauri/src/util.rs) 新增 `#[cfg(test)] mod tests`，添加 `apply_runtime_overrides_*` 测试：`PAKE_RUNTIME_URL` 设置时 `windows[0].url` 被覆盖且 `url_type=="web"`；未设置时不变

### Implementation for User Story 1

- [x] T012 [US1] 在 [src-tauri/src/main.rs](../../src-tauri/src/main.rs) 抽出可测函数 `fn parse_runtime_args<I: IntoIterator<Item = String>>(args: I) -> Result<(Option<String>, bool), String>`，返回 `(runtime_url, ignore_cert)`；`main()` 调用并把结果用 `env::set_var` 写入 `PAKE_RUNTIME_URL` / `PAKE_IGNORE_CERT`，错误时 `eprintln!` + `exit(1)`（覆盖现有零散逻辑，保持向后兼容的错误信息文案）
- [x] T013 [US1] 在 [src-tauri/src/util.rs](../../src-tauri/src/util.rs) 把现有"读 `PAKE_RUNTIME_URL` → 覆盖 `windows[0].url`/`url_type='web'`"逻辑重构为独立函数 `fn apply_runtime_overrides(cfg: &mut PakeConfig)`，由 `get_pake_config()` 在 mut 解析后调用；保持现有运行时行为（无新依赖）
- [x] T014 [US1] 在 [bin/cli.ts](../../bin/cli.ts) / [README.md](../../README.md) 用法段落补齐 `--url` 在打包后二进制可用的说明（CLI build 命令本身**不**新增 `--url`，因其只对运行时有效）；同步更新 [docs/cli-usage.md](../../docs/cli-usage.md) / [docs/cli-usage_CN.md](../../docs/cli-usage_CN.md) 新章节"运行时参数"

**Checkpoint**: User Story 1 端到端可独立验证（quickstart §3）。

---

## Phase 4: User Story 2 — 自定义证书 SSL 容忍（Priority: P1）

**Goal**: 提供显式 `--ignore-cert` 运行时开关 + 沿用 `pake.json.ignore_certificate_errors` 构建期开关，二者 OR 合并。废除 localhost 隐式启用。

**Independent Test**: 自签 HTTPS 站点 `--ignore-cert` 启动可加载；不带开关时 WebView 显示证书错误页；公网正常证书站点行为不变；`pake.json` 中静态启用同样生效。

### Tests for User Story 2

- [x] T015 [P] [US2] 在 T010 同一测试模块中追加 `parse_runtime_args_ignore_cert_*` 测试：命中 `--ignore-cert` 时 `ignore_cert==true`；未命中时 false；`--url` 与 `--ignore-cert` 任意顺序组合均正确
- [x] T016 [P] [US2] 在 T011 同一测试模块中追加 `apply_runtime_overrides_ignore_cert_*` 测试：`PAKE_IGNORE_CERT=1` 时 `windows[0].ignore_certificate_errors` 被 OR 为 true；未设且 `pake.json` 已为 true → 仍为 true（不被关闭）

### Implementation for User Story 2

- [x] T017 [US2] 扩展 T012 的 `parse_runtime_args`：识别无值 flag `--ignore-cert`；命中时返回 `ignore_cert=true`
- [x] T018 [US2] 扩展 T013 的 `apply_runtime_overrides`：当 `PAKE_IGNORE_CERT` 被设置（任意非空值）时，对 `windows[0].ignore_certificate_errors` 执行 `|= true`（不覆盖已为 true 的构建期值）
- [x] T019 [US2] 在 [docs/advanced-usage.md](../../docs/advanced-usage.md) / [docs/advanced-usage_CN.md](../../docs/advanced-usage_CN.md) 新增"自签名/内部 CA 证书"小节：说明 `--ignore-cert` 与 `pake.json` 字段的关系、安全提示（默认安全 / 显式启用）、仅本次进程有效；附 quickstart §4 示例

**Checkpoint**: User Story 2 端到端可独立验证（quickstart §4），与 US1 不冲突。

---

## Phase 5: User Story 3 — Ubuntu 24.04 专属 deb 构建（Priority: P2）

**Goal**: 一条命令 / 一条 GHA workflow 在 Ubuntu 24.04 (x86_64) 仅产出单一 `*_amd64.deb`，依赖与 24.04 默认源兼容，**不**破坏现有跨平台路径。

**Independent Test**: 在 Ubuntu 24.04 容器执行 `pnpm run build:ubuntu24 -- https://example.com --name browser` → `src-tauri/target/release/bundle/deb/browser_*_amd64.deb` 唯一一个文件；`dpkg -I` 显示 `Architecture: amd64` + 24.04 兼容依赖；干净 24.04 容器 `apt install` 通过。

### Tests for User Story 3

- [ ] T020 [P] [US3] 创建 [tests/unit/linux-builder-ubuntu24.test.ts](../../tests/unit/linux-builder-ubuntu24.test.ts)，使用 vitest 验证：当 `process.env.PAKE_UBUNTU24='1'` 且 `platform==='linux'` 时，`new LinuxBuilder({...targets:'deb,appimage'})` 实例的内部 `targets` 被强制为 `'deb'`、`buildArch==='x64'`；当 `PAKE_UBUNTU24` 未设置时行为完全不变（回归）

### Implementation for User Story 3

- [ ] T021 [P] [US3] 修改 [bin/builders/LinuxBuilder.ts](../../bin/builders/LinuxBuilder.ts) 构造函数：识别 `process.env.PAKE_UBUNTU24 === '1'`，命中时强制 `this.options.targets = 'deb'`、`this.buildFormat = 'deb'`、`this.buildArch = 'x64'`；记录 `this.ubuntu24Only = true` 实例字段供后续使用
- [ ] T022 [P] [US3] 修改 [bin/helpers/tauriConfig.ts](../../bin/helpers/tauriConfig.ts) 合并 `tauri.linux.conf.json` 阶段：当 `process.env.PAKE_UBUNTU24 === '1'` 时，深合并 `bundle.active=true`、`bundle.targets=['deb']`、`bundle.linux.deb.depends=["libwebkit2gtk-4.1-0","libgtk-3-0","libayatana-appindicator3-1","curl","wget"]`；不影响其他平台合并
- [ ] T023 [US3] 在 [package.json](../../package.json) `scripts` 段新增 `"build:ubuntu24": "cross-env PAKE_UBUNTU24=1 node ./bin/cli.js"`（`cross-env@^10.1.0` 已在 devDependencies，可直接复用与现有 `cli`/`cli:build` 一致的写法）；脚本 MUST 在非 Linux 平台运行时被 LinuxBuilder/cli 入口检测并友好报错（沿用 BuilderProvider 现有平台分发）
- [ ] T024 [US3] 创建 [.github/workflows/build-ubuntu24.yml](../../.github/workflows/build-ubuntu24.yml)：`runs-on: ubuntu-24.04`，触发 `workflow_dispatch` + `push: tags: ['ubuntu24-v*']`，输入 `url`(required) / `name`(default: `browser`)；workflow 必须在调用 build 之前用 [bin/utils/name.ts](../../bin/utils/name.ts) 的 `generateLinuxPackageName` 等价正则 `^[a-z0-9][a-z0-9-]*$` 校验 `name` 输入，不合法直接 fail-fast 并提示规则；单 job：checkout → `./.github/actions/setup-env` → `pnpm install --frozen-lockfile` → 记录 `start_ts=$(date +%s)` → `pnpm run build:ubuntu24 -- "${{ inputs.url }}" --name "${{ inputs.name }}"` → 记录 `end_ts` 并把 `BUILD_DURATION_SEC` 写入 `$GITHUB_STEP_SUMMARY`（供 SC-004 度量） → `actions/upload-artifact@v4` 命名 `${{ inputs.name }}-ubuntu24-amd64-deb` 上传 `src-tauri/target/release/bundle/deb/*_amd64.deb`
- [ ] T025 [US3] 在 [docs/github-actions-usage.md](../../docs/github-actions-usage.md) / [docs/github-actions-usage_CN.md](../../docs/github-actions-usage_CN.md) 新增"Ubuntu 24.04 专属构建"小节，说明触发方式、产物命名、与 release.yml 的关系（互不干扰）

**Checkpoint**: User Story 3 可独立验证（quickstart §2）；US1/US2 在该 deb 上同样可用（quickstart §3/§4）。

---

## Phase 6: Polish & Cross-Cutting Concerns

- [ ] T026 [P] 运行 quickstart.md §2 / §3 / §4 / §5 全部场景，记录截屏与命令输出到 [specs/001-cli-url-ssl-ubuntu/quickstart-evidence.md](./quickstart-evidence.md)（仅本地，不入库 PR）；额外在 macOS 本地用 `cargo run -- --url https://example.com` 烟测一次 US1，覆盖 spec Edge Cases 中"非 Linux 启动带 --url"场景
- [ ] T027 [P] 在 macOS 与 Windows 本地分别运行 `pnpm run build:mac` / `pnpm run build:win`，确认 FR-008 跨平台路径未被破坏；记录 dmg / msi 产物大小与 baseline 比较（守 SC-005 ±5%）
- [ ] T028 在 [README.md](../../README.md) / [README_CN.md](../../README_CN.md) "运行" / "构建" 段落追加：运行时 `--url` / `--ignore-cert`、`build:ubuntu24` 一段简介 + 链接到 [docs/cli-usage.md](../../docs/cli-usage.md) 与 [docs/advanced-usage.md](../../docs/advanced-usage.md) 详细章节；同时在 [CONTRIBUTING.md](../../CONTRIBUTING.md) 或 release notes 草稿中显式声明 FR-005 的破坏性变更（移除 localhost 隐式启用）
- [ ] T029 重新运行 `cd src-tauri && cargo check && cargo test --lib` 与 `pnpm test`（项目当前 [package.json](../../package.json) 无 `lint` script，prettier/eslint 由 pre-commit 处理，本任务不涉及）确认零回归
- [ ] T030 自检对照宪章 [.specify/memory/constitution.md](../../.specify/memory/constitution.md) §13（无新 crate / 无 opt-level 改动 / deb 体积偏差 ≤ ±5%）、§15（无隐式 SSL 旁路 / capabilities 未放宽）、§14（无 `.unwrap()` 新增 / 无 setup 期阻塞 IO）；任何不符须立即修复或回到 plan 修订
- [ ] T031 [P] **SC-004 度量**：取 GHA build-ubuntu24 工作流首次成功运行的 `BUILD_DURATION_SEC`，对比同 commit 在 [release.yml](../../.github/workflows/release.yml) Linux job 的总耗时（取 actions UI 历史最近 1 次为参考），写入 [quickstart-evidence.md](./quickstart-evidence.md) 表格；若比值 > 60% 触发 SC-004 失败，回到 plan 调整
- [ ] T032 [P] **SC-005 度量**：执行 `ls -la src-tauri/target/release/bundle/deb/browser_*_amd64.deb` 取本特性产物大小，与 T001 记录的 baseline deb 大小对比；偏差 > ±5% 视为 SC-005 失败，结论写入 [quickstart-evidence.md](./quickstart-evidence.md)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**：无依赖，立即开始
- **Phase 2 (Foundational)**：依赖 Phase 1；**阻塞 US1/US2/US3**
- **Phase 3–5 (User Stories)**：均依赖 Phase 2；**彼此可并行**（不同文件 / 不同模块；US1/US2 共享 `parse_runtime_args` 与 `apply_runtime_overrides`，按下方顺序合并）
- **Phase 6 (Polish)**：依赖所有 US 完成

### User Story Dependencies

- **US1 (P1)**：依赖 Phase 2；**MVP**
- **US2 (P1)**：依赖 Phase 2；**与 US1 共用 `parse_runtime_args`/`apply_runtime_overrides`**：建议先合 US1（T012/T013），再做 US2 的 T017/T018 增量；测试 T015/T016 可与 T010/T011 同测试模块平行编写
- **US3 (P2)**：依赖 Phase 2，但**与 US1/US2 完全正交**（TS Builder 与 GHA workflow），可由不同人完全并行

### Within Each Story

- Tests 文件创建 ([P]) 与实现可并行编写，但实现完成时所有相关测试 MUST 通过
- 文档 (T014/T019/T025) 在实现完成后更新

### Parallel Opportunities

- **Phase 1**：T002 与 T001 完成后即可进行（仅文档追记，无依赖）
- **Phase 2**：T004（Cargo.toml）/ T005（lib.rs）/ T007（invoke.rs）/ T008（util.rs）涉及不同文件，可并行编辑（注意 T005 与 T006 都改 `lib.rs`，先 T005 后 T006，避免冲突）
- **US1**：T010 / T011 并行；T012 / T013 顺序（T013 需要 T012 引入的 env 变量名常量保持一致 — 可共定义后并行）
- **US2**：T015 / T016 并行；T017 / T018 顺序在 US1 之后
- **US3**：T020 / T021 / T022 三人三件事完全并行；T023 依赖 T021 的环境变量约定；T024 依赖 T023 的 npm script
- **Polish**：T026 / T027 / T028 全部 [P]

---

## Parallel Example: User Story 3 三人并行启动

```bash
# 开发者 A — TS Builder 修改
$ git checkout -b us3-builder
# 完成 T021 → push & PR

# 开发者 B — Tauri 配置合并
$ git checkout -b us3-config
# 完成 T022 → push & PR

# 开发者 C — 测试先行
$ git checkout -b us3-test
# 完成 T020 → push（先标记 skip 或预期失败，待 A/B 合后转绿）
```

三个分支合并到 `001-cli-url-ssl-ubuntu` 后，再做 T023（package.json）→ T024（workflow）→ T025（docs）顺序串行。

---

## Implementation Strategy: 增量交付

| 里程碑 | 包含任务 | 价值 |
|---|---|---|
| **M0 — 可编译基线** | Phase 1 + Phase 2 | 修复阻断、剥离 F4 噪声、移除 §15 违规 |
| **M1 — MVP（US1）** | M0 + Phase 3 | 同一二进制多环境复用，最高 ROI |
| **M2 — 内网完备（US1+US2）** | M1 + Phase 4 | 内网 / 自签证书全场景可用 |
| **M3 — 工程效率（+US3）** | M2 + Phase 5 | CI 时间砍 60%，单一交付物 |
| **M4 — 生产就绪** | M3 + Phase 6 | 文档、跨平台回归、宪章自检 |

每个里程碑都是一个独立可发布的增量；建议至少在 **M0** 与 **M1** 之间各打一次内部 dogfood 标签。

---

## 任务总数：31

| Phase | 任务数 |
|---|---|
| 1. Setup | 2 |
| 2. Foundational | 6 |
| 3. US1 (P1) | 5 |
| 4. US2 (P1) | 5 |
| 5. US3 (P2) | 6 |
| 6. Polish | 7 |
| **合计** | **31** |

**MVP 范围**：T001–T014（13 个任务，覆盖到 US1 完成）。
