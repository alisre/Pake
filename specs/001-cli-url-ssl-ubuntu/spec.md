# Feature Specification: CLI `--url` 运行时覆盖 + 自定义证书 SSL 容忍 + Ubuntu 24.04 专属构建

**Feature Branch**: `001-cli-url-ssl-ubuntu`
**Created**: 2026-04-30
**Status**: Draft
**Input**: User description: "1. 自定义参数 --url 传递访问的网址；2. 如果是自定义的证书，忽略 SSL 证书告警；3. 打包的时候仅需要运行在 Ubuntu 24.04 上的二进制文件"

## User Scenarios & Testing *(mandatory)*

### User Story 1 — 运维人员通过 `--url` 在运行时切换目标站点 (Priority: P1)

运维同事在内网部署了已打包的 Pake 二进制（如 `browser`），不重新构建即可让同一个二进制在不同环境（开发/预发/生产）指向不同后台地址。

**Why this priority**: 这是核心交付物。如果只有它落地，二进制即可在多环境复用，避免为每个环境重建一次（构建一次约 5–10 分钟），是用户最直接的价值。

**Independent Test**: 在终端执行 `./browser --url https://stage.intra.example.com`，窗口应加载该 URL 而非 `pake.json` 中的编译期 URL；不带参数时回退到编译期 URL。

**Acceptance Scenarios**:

1. **Given** 已打包的二进制 `browser` 内置 `pake.json` 默认 URL `https://prod.example.com`，**When** 用户运行 `./browser --url https://stage.intra.example.com`，**Then** 窗口加载 `https://stage.intra.example.com`，且关闭后再次无参启动仍回到默认 URL（不持久化）
2. **Given** 同上，**When** 用户运行 `./browser`（不带 `--url`），**Then** 窗口加载 `pake.json` 中的默认 URL
3. **Given** 同上，**When** 用户运行 `./browser --url ftp://foo`（非 http/https），**Then** 进程以非零退出码退出并输出明确错误：`--url 必须以 http:// 或 https:// 开头`

---

### User Story 2 — 访问自签名/内网证书站点不被 SSL 告警阻断 (Priority: P1)

用户的目标 Web 站点使用内部 CA 或自签名证书；Pake 打包后访问应直接放行，不弹证书错误页/白屏。

**Why this priority**: 内网场景的强阻塞问题。没有这条 P1，故事 1 在生产以外的内网场景会大概率失败（白屏）。与 P1 故事 1 并列必备。

**Independent Test**: 在本地用 mkcert 或 openssl 起一个自签名 https 服务，`./browser --url https://localhost:8443`，应直接渲染页面，不出现证书错误页。

**Acceptance Scenarios**:

1. **Given** 二进制以 `--ignore-cert` 启动并指向自签 HTTPS 服务，**When** WebView 发起首屏请求，**Then** 页面正常渲染，无 SSL 错误中间页
2. **Given** 二进制以默认参数启动并指向公网正常证书的 HTTPS 站点，**When** 加载页面，**Then** 行为与未启用本特性前一致（证书校验仍然生效）
3. **Given** `pake.json` 中 `ignore_certificate_errors: true`（构建期开启），**When** 启动后访问任意 HTTPS 站点，**Then** WebView 全程跳过证书校验

---

### User Story 3 — 一条命令仅产出 Ubuntu 24.04 可执行二进制 (Priority: P2)

打包者只关心 Linux x86_64（Ubuntu 24.04）目标，不需要 macOS/Windows，也不需要 ARM/RPM/AppImage，需要最快路径产出可发布产物。

**Why this priority**: 是工程效率优化。P1 两项落地后，没有 P2 也能用（只是 CI/手动构建会做无效工作）。但 P2 可显著缩短 CI 时间并减小制品矩阵。

**Independent Test**: 在 Ubuntu 24.04 上运行项目内单一命令（如 `pnpm run build:ubuntu24` 或等价 GitHub Actions 工作流）后，`src-tauri/target/release/bundle/deb/` 下产出唯一一个 `*_amd64.deb` 文件，且 `dpkg -I` 显示 `Architecture: amd64`、依赖与 Ubuntu 24.04 (`libwebkit2gtk-4.1-0`) 兼容。

**Acceptance Scenarios**:

1. **Given** 干净的 Ubuntu 24.04 (x86_64) 环境，**When** 执行约定的构建命令，**Then** 仅产出 `.deb`（不产出 AppImage/RPM/macOS/Windows 包）
2. **Given** 该 `.deb` 包，**When** 在另一台干净 Ubuntu 24.04 机器执行 `sudo apt install ./browser_*_amd64.deb`，**Then** 安装成功，应用启动并通过故事 1 的场景 1
3. **Given** 该构建命令，**When** 在 macOS/Windows 上调用，**Then** 应明确报错或被跳过，不污染本地构建目录

### Edge Cases

- `--url` 与位置参数 URL 同时提供（`./browser https://a --url https://b`）：以 `--url` 为准，并在日志（debug 模式）告知覆盖关系
- `--url` 值含查询串 / 锚点 / Unicode 域名：完整透传给 WebView，不做编码改写
- 站点跳转（302）到另一个证书无效域：`--ignore-cert` / `ignore_certificate_errors` 也对跳转后域生效（与 Chromium `--ignore-certificate-errors` 行为一致）
- 用户在 macOS/Windows 上启动带 `--url` 的二进制：应同样工作（虽然本特性 P2 只交付 Ubuntu 二进制，运行时能力对所有平台都已启用）
- Ubuntu 24.04 之外的 Linux（如 Debian 12、Ubuntu 22.04）安装 deb：不在保证范围；`dpkg` 依赖检查可能失败，文档需说明

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: 二进制 MUST 接受 `--url <URL>` 命令行参数，URL 协议必须为 `http` 或 `https`，否则以非零退出码退出并打印明确错误
- **FR-002**: 当 `--url` 提供时，二进制 MUST 在加载窗口前用该 URL 覆盖 `pake.json` 中 `windows[0].url` 与 `url_type=web`，本次进程内有效，不持久化到磁盘
- **FR-003**: 二进制 MUST 提供 `--ignore-cert`（或等价显式开关）启用 WebView 的 `--ignore-certificate-errors`；该开关默认关闭
- **FR-004**: `pake.json` 既有的 `ignore_certificate_errors: bool` 字段 MUST 继续生效（编译期开启）；命令行开关与配置项任一为真即生效
- **FR-005** *(BREAKING CHANGE)*: 系统 MUST 移除"访问 `localhost`/`127.0.0.1` 自动忽略证书"的隐式行为（避免与显式开关语义冲突，安全语义需用户显式选择）。属用户可见行为破坏，CHANGELOG 与 release notes MUST 显式声明
- **FR-006**: 项目 MUST 提供一条单一命令/工作流，在 Ubuntu 24.04 (x86_64) 上仅产出 `*_amd64.deb`，不触发 AppImage/RPM/ARM/macOS/Windows 构建
- **FR-007**: 该 deb 包 MUST 在干净 Ubuntu 24.04 上 `apt install` 成功；运行时依赖 MUST 与 Ubuntu 24.04 默认源兼容，至少声明：`libwebkit2gtk-4.1-0`、`libgtk-3-0`、`libayatana-appindicator3-1`、`curl`、`wget`
- **FR-008**: 现有 macOS/Windows/Linux 多目标构建路径 MUST 继续可用，不能被本特性破坏（仅新增"Ubuntu-24.04-only"路径）
- **FR-009**: 当前分支上已存在的原型代码缺陷 MUST 修复：`src-tauri/Cargo.toml` 中 `tokio`/`chrono`/`tauri` 三个依赖被压成单行需恢复换行；`src-tauri/src/lib.rs` 中被破坏的 `.on_window_event(...)` 起始行需恢复
- **FR-010**: 与本特性无关的 F4 "Linux perf 监控" 改动（`get_perf_stats` invoke、60 秒采样写 perf.log、`chrono` 依赖）MUST 从本分支移除或拆分到独立 spec，避免与本特性范围交织

### Key Entities

- **Runtime URL Override**: 进程级、易失，仅本次启动有效；优先级高于 `pake.json` 编译期 URL；不写盘
- **SSL Trust Policy**: 三态来源（无开关、CLI `--ignore-cert`、`pake.json.ignore_certificate_errors`），任一为真即对所有 WebView 请求生效；对所有平台采用 Chromium `--ignore-certificate-errors` 等价机制
- **Ubuntu 24.04 Build Artifact**: 单一 `.deb` 文件，amd64 架构，依赖 `libwebkit2gtk-4.1-0` 等 Ubuntu 24.04 默认包

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 同一个二进制通过 `--url` 在 3 个不同后台地址间切换均成功加载，无需重建（从首次需求到验证完成 < 1 分钟/次）
- **SC-002**: 自签名 HTTPS 站点首屏加载成功率 100%（10 次冷启动测试）
- **SC-003**: 默认证书校验回归零（公网 HTTPS 站点 10 次抽样均正常）
- **SC-004**: Ubuntu 24.04 专属构建端到端时间 ≤ 现有"Linux 全套"构建时间的 60%（去掉 AppImage/RPM/ARM 后）。度量主体：[.github/workflows/build-ubuntu24.yml](../../.github/workflows/build-ubuntu24.yml) 单 job 实际耗时与同一 commit 下 [release.yml](../../.github/workflows/release.yml) Linux 矩阵 job 平均耗时对比，结果归档到 [quickstart-evidence.md](./quickstart-evidence.md)（人工记录，不作 CI 强约束）
- **SC-005**: 产出的 `.deb` 文件大小相比 3.11.x 同 commit 基线偏差 ≤ ±5%（守住 Pake 体积红线）。度量主体：本特性合并前最近一次 main 上的 `*_amd64.deb` 大小（`ls -la`），实现完成后由 Phase 6 任务记录
- **SC-006**: 在干净 Ubuntu 24.04 容器内 `apt install ./browser_*_amd64.deb` → 启动 → 加载 `--url` 给定页面，三步均成功（CI 中可复现）

## Assumptions

- 用户接受"忽略证书"必须**显式**启用（CLI 开关或 `pake.json` 字段），不接受隐式按域名启用的方案
- 架构与产物形态见 FR-006（不在此重复）
- 已经合入分支但与本特性无关的 F4 perf 监控改动允许从本分支剥离（移到独立分支或丢弃）
- macOS/Windows 在本特性下**仍可本地构建**，但不在 P1/P2 验收范围内
