# Phase 0 — Research

## R-001 运行时参数解析方式

- **Decision**: 沿用 `std::env::args` 手工扫描（已在 [src-tauri/src/main.rs](../../src-tauri/src/main.rs) 现状）
- **Rationale**:
  - 仅 2 个参数（`--url`、`--ignore-cert`），手解析 < 30 行 Rust，零依赖
  - 守 [pake.speckit.constitution](../../pake.speckit.constitution) §13 "拒绝引入 >500KB 的纯 JS 运行时依赖；Rust 依赖优先 default-features = false"
  - Tauri 主进程在 `main.rs` 解析后用 `env::set_var` 透传给 `lib.rs::run`，避免改 `app_lib::run` 签名
- **Alternatives considered**:
  - `clap`：功能强但 release 二进制体积约 +120KB，且会带 `regex`/`anstyle` 等传递依赖 — 拒绝
  - `pico-args`：体积友好（< 10KB），但本场景仍属过度工程 — 拒绝
  - 通过 Tauri `tauri-plugin-cli`：默认隐藏在前端 invoke 之后，启动期取不到 — 与"启动前覆盖 URL"的时机不匹配，拒绝

## R-002 运行时 URL 覆盖的注入点

- **Decision**: `main.rs` 解析 → `env::set_var("PAKE_RUNTIME_URL", url)` → `util.rs::get_pake_config()` 在解析 `pake.json` 后立即用 env 覆盖 `PakeConfig.windows[0].url` 与 `url_type="web"`
- **Rationale**:
  - `get_pake_config` 是配置的**单一入口**（[src-tauri/src/util.rs](../../src-tauri/src/util.rs)），所有窗口构建都从这里取值，覆盖一处全局生效
  - 用 env 而非全局 `static` 解耦 main / lib，符合 §3 "禁止 static mut 持有 Tauri 对象"的精神
  - 不写盘 → 满足 spec FR-002 "本次进程内有效，不持久化"
- **Alternatives considered**:
  - 在 `main.rs` 直接 mutate 全局 `OnceCell<PakeConfig>`：跨模块状态、不易测试，拒绝
  - 让 TS CLI 在打包时把 URL 烘进 `pake.json`：变成构建期参数，丢失"同一个二进制多环境复用"的核心价值，拒绝
  - 用 `tauri::Builder::setup` 里 reload：webview 已经按旧 URL 创建，会出现"加载 → 跳转"的闪烁，拒绝

## R-003 自定义证书 SSL 绕过的实现

- **Decision**: 复用 Pake 已有的 `WindowConfig.ignore_certificate_errors`（[src-tauri/src/app/config.rs:36](../../src-tauri/src/app/config.rs)）+ [window.rs:292](../../src-tauri/src/app/window.rs) 现有的三平台 Chromium `--ignore-certificate-errors` 注入逻辑；CLI 新开关 `--ignore-cert` 通过 `env::set_var("PAKE_IGNORE_CERT","1")` → `get_pake_config` 用 OR 合并到 `windows[0].ignore_certificate_errors`
- **Rationale**:
  - 既有实现已覆盖 Linux/Windows 的 `additional_browser_args` 与 macOS 的 `WindowBuilder::additional_browser_args` 三种路径，**无需新代码**
  - 三态合并（构建期 `pake.json` || 运行期 `--ignore-cert`）语义清晰，OR 即可
  - 等价于 Chromium `--ignore-certificate-errors`，与浏览器开发者熟悉的语义一致
- **Alternatives considered**:
  - 自定义证书校验回调（接受指定 CA / 指定 fingerprint）：实现复杂、跨 webview 平台不一致（WebKitGTK / WebView2 / WKWebView API 差异大），且 spec 未要求，拒绝
  - 仅对 `localhost`/`127.0.0.1` 隐式启用（用户原型中已有）：违反 [pake.speckit.constitution](../../pake.speckit.constitution) §15 "最小权限 / 显式启用"原则；用户在内网（10.x、自定义域名）反而无法享用 — **必须移除**

## R-004 Ubuntu 24.04 专属构建路径

- **Decision**:
  1. 新增 `pnpm run build:ubuntu24` → 等价于 `pake <url> --targets deb --name <name>`，由 [LinuxBuilder.ts](../../bin/builders/LinuxBuilder.ts) 在内部识别该模式（环境变量 `PAKE_UBUNTU24=1`）后强制单一 deb 流程
  2. 在 `bin/helpers/tauriConfig.ts` 合并阶段，当 `PAKE_UBUNTU24=1` 时向 `tauri.linux.conf.json` 注入 `bundle.active=true` / `bundle.targets=["deb"]` / `bundle.linux.deb.depends=["libwebkit2gtk-4.1-0","libgtk-3-0","libayatana-appindicator3-1","curl","wget"]`
  3. 新增 [.github/workflows/build-ubuntu24.yml](../../.github/workflows/build-ubuntu24.yml)：`runs-on: ubuntu-24.04`，单 job、单产物
- **Rationale**:
  - 用环境变量驱动而非新 Builder 类，**不**破坏既有 Mac/Win/Linux 多 target 构建路径（spec FR-008）
  - `libwebkit2gtk-4.1-0` 是 Ubuntu 24.04 默认源里的版本（22.04 是 4.0），显式声明保证 `apt install` 时的依赖正确
  - 单 GHA workflow 仅跑一个 ubuntu-24.04 runner job，相比现有 `release.yml` 的 macOS+Windows+Linux 矩阵节省 60–70% 时间
- **Alternatives considered**:
  - 删除 `tauri.macos.conf.json` / `tauri.windows.conf.json`：不可逆、破坏上游同步、违反 FR-008，拒绝
  - 把 deb 的 `depends` 写死在 `tauri.linux.conf.json`：影响其他 Linux 发行版用户（如 Debian 12 也用此分支构建），拒绝；改为运行时按 `PAKE_UBUNTU24` 注入
  - 改 `runs-on: ubuntu-latest`：GHA 滚动升级会让"Ubuntu 24.04 专属"承诺漂移，拒绝；显式锁 `ubuntu-24.04`

## R-005 已有原型缺陷

当前 `001-cli-url-ssl-ubuntu` 分支（沿用 main 上的未提交修改）已有以下**阻断性问题**，必须在实现阶段一并修复：

| 文件 | 问题 | 影响 |
|---|---|---|
| [src-tauri/Cargo.toml](../../src-tauri/Cargo.toml) | `tokio = {...}chrono = {...}tauri = {...}` 三个依赖被压成单行（无换行） | `cargo` 解析失败，无法构建 |
| [src-tauri/src/lib.rs](../../src-tauri/src/lib.rs) | 删除了 `.on_window_event(move |_window, _event| {` 起始行，导致后续 `if let WindowEvent::CloseRequested` 块语法错误 | 编译失败 |
| 同上 | 引入 `chrono` 用于 F4 perf 监控，与本特性正交 | 体积+依赖噪声 |
| [src-tauri/src/app/invoke.rs](../../src-tauri/src/app/invoke.rs) | `get_perf_stats` invoke 命令属于 F4 | 与本特性正交 |
| [src-tauri/src/util.rs](../../src-tauri/src/util.rs) | `host=="localhost"` 隐式启用 ignore_certificate_errors | 违反 §15 |

- **Decision**: F4 改动从本分支剥离到独立 spec `002-linux-perf-monitor`；其余 4 处缺陷在本特性的实现任务里一并修复
- **Rationale**: 让本 PR 的 diff 聚焦三件事，便于 review；F4 监控是有价值的功能但需独立 spec/clarify
- **Baseline reference**: F4 perf 监控代码片段保留于 baseline commit `cb911ec7da8931cfac59c0c262da70920006071d`（git working tree 中尚未提交的本分支修改）；未来独立 spec 可通过 `git show cb911ec:src-tauri/src/lib.rs` 与 `git show cb911ec:src-tauri/src/app/invoke.rs` 在 git 历史中恢复 F4 原型

## R-006 Tauri 2 / WebKitGTK on Ubuntu 24.04

- **Decision**: 不升级 Tauri，沿用 2.10.2；deb depends 锁 `libwebkit2gtk-4.1-0`
- **Rationale**:
  - Tauri 2 自 2.0 起就用 webkit2gtk-4.1 系（Ubuntu 24.04 自带）
  - 沿用 [pake.speckit.constitution](../../pake.speckit.constitution) §12 "WEBKIT_DISABLE_DMABUF_RENDERER=1 禁止删除"的现有保护
- **Alternatives**: 升 Tauri 到最新 2.x — 与本特性无关，单独决策

---

**Status**: 所有 NEEDS CLARIFICATION 已解决。可进入 Phase 1。
