# Pake SpecKit Constitution

> 版本: 1.0.0
> 适用范围: Pake 项目 (https://github.com/tw93/Pake)
> 技术栈: Tauri 2 + wry + WebKitGTK/WebView2/WKWebView + TypeScript CLI
> 来源: 基于 Rust Skills (m01-m15, domain-*) 并裁剪/扩展为 Pake 专属约束

---

## §0 项目定位 (Project Identity)

Pake 是一个 **将任意网页打包为轻量桌面应用** 的工具：

- **CLI 层** (TypeScript / Node.js, [bin/cli.ts](bin/cli.ts)): 用户入口，组装配置、调用 Tauri 构建
- **运行时层** (Rust + Tauri 2, [src-tauri/src/lib.rs](src-tauri/src/lib.rs)): 实际桌面应用，承载 webview、菜单、托盘、快捷键
- **注入层** (JavaScript/CSS, [src-tauri/src/inject/](src-tauri/src/inject/)): 进入网页运行时改造体验

**核心约束**：
1. 体积小 (~5MB)、启动快、内存低 ← 所有改动 MUST 守住这条线
2. 跨平台一致 (macOS / Windows / Linux) ← 任何平台特性 MUST 用 `#[cfg]` 隔离
3. 用户零配置可用 ← 默认值优于配置项

---

## §1 三层认知框架 (Cognitive Layers)

排查问题或设计功能时，先定位层级：

```
Layer 3: Pake 业务约束 (WHY)
├── 体积/性能/跨平台一致性
├── 一键打包、零配置体验
└── "为什么 Pake 要这样？"

Layer 2: Tauri/wry 设计选择 (WHAT)
├── webview 生命周期、IPC、配置合并
├── §10 (Tauri 模式) / §11 (注入策略)
└── "Tauri 该用什么模式？"

Layer 1: 语言机制 (HOW)
├── Rust: 所有权/借用/生命周期/trait
├── TS: 类型/异步/模块
└── "如何在代码里实现？"
```

**路由规则**:
- 编译错误 / panic → Layer 1 向上追溯
- 跨平台行为差异 → Layer 2 检查 `#[cfg]` 与配置合并
- 用户体验问题 → Layer 3 回到 Pake 定位

---

## §2 所有权与借用 (Ownership in Tauri)

### Pake 特有场景
| 数据 | 所有权模式 | 原因 |
|------|-----------|------|
| `PakeConfig` (启动期解析) | clone 到各组件 | 配置是值对象，体积小 |
| `MultiWindowState` | `tauri::State<Arc<...>>` | 跨 invoke handler 共享 |
| `tauri::Window` / `WebviewWindow` | clone (内部 Arc) | Tauri 句柄本身就是 Arc |
| `AppHandle` | clone | 同上，`Clone` 廉价 |
| 注入的 JS/CSS 字符串 | `&'static str` (include_str!) | 编译期内联，零分配 |

### 错误 → 设计问题
| 编译错误 | 不要只是 | 先问 |
|---------|---------|------|
| E0382 (Tauri handle moved) | "加 .clone()" | 是否在 spawn 闭包中 — 应该在闭包外 clone |
| E0597 (window 引用过期) | "改生命周期" | 是否应该用 AppHandle 重新 get_webview_window |
| E0277 Send (跨 await 持锁) | "换 lock" | 该锁是否应跨 .await — 通常应该缩短作用域 |

**规则**：
- Tauri handle (`AppHandle`/`Window`/`WebviewWindow`) 在 `tauri::async_runtime::spawn` 之前 MUST clone
- 禁止在 `.await` 跨越点持有 `std::sync::Mutex` guard

---

## §3 智能指针与状态管理 (State Management)

### Pake 状态决策树
```
需要在 invoke handler 间共享？
├─ 是 → tauri::State<T>，T 内部用 Arc<Mutex<...>> 或 Arc<RwLock<...>>
└─ 否 → 闭包捕获即可
```

### 当前实践 (参考 [src-tauri/src/app/window.rs](src-tauri/src/app/window.rs))
```rust
pub struct MultiWindowState { /* Arc<Mutex<...>> 字段 */ }
app.manage(MultiWindowState::new(pake_config.clone(), tauri_config.clone()));
```

### 规则
- 全局可变状态 MUST 通过 `app.manage()` 注册
- 禁止使用 `static mut` / `lazy_static` 持有 Tauri 相关对象
- 多 window 场景 MUST 通过 `MultiWindowState` 集中管理，避免 window 句柄散落

---

## §4 可变性 (Mutability)

- 默认 `let`，确实需要才 `mut`
- 跨线程 / 跨 handler 可变共享 → `Arc<Mutex<T>>`
- TS 端配置对象 ([bin/helpers/merge.ts](bin/helpers/merge.ts)) MUST 走纯函数合并，不就地修改入参

---

## §5 泛型与 Trait

Pake 实际泛型使用较少，但 Builder 抽象是典型场景：

参考 [bin/builders/BaseBuilder.ts](bin/builders/BaseBuilder.ts) + Mac/Win/Linux 三个具体类。

### 规则
- 平台 Builder MUST 继承 `BaseBuilder`，覆盖差异，复用共性
- Rust 端如需新增插件，优先用 `tauri::plugin::Builder` 的现成 trait 而非自造

---

## §6 错误处理 (Error Handling)

### Rust 端 (src-tauri)
| 场景 | 做法 |
|------|------|
| Tauri invoke handler 返回 | `Result<T, String>` 或自定义错误（必须 Serialize） |
| 启动期不可恢复（找不到 config 等） | `expect("明确的用户友好原因")` |
| 平台特定失败（剪贴板/通知失败） | 记录日志后降级，**不要 panic** |
| 网络下载 ([invoke.rs](src-tauri/src/app/invoke.rs)) | 返回 `Result`，让前端展示 |

### TS 端 (bin/)
| 场景 | 做法 |
|------|------|
| CLI 用户输入校验 | 抛错并 `process.exit(1)`，给清晰中英文提示 |
| 子进程 (cargo/tauri build) 失败 | 透传退出码 + 末尾 50 行日志 |
| 文件 I/O | try/catch + 友好错误信息 |

### 禁止
- 生产 Rust 代码出现 `.unwrap()` 除非紧邻的代码已保证不变量
- TS 用 `any` 静默错误
- 吞掉错误后继续往下走（必须 log + 处理 / 重试 / 失败）

---

## §7 并发 (Concurrency)

### Pake 并发场景
| 场景 | 工具 |
|------|------|
| 窗口异步显示 / fullscreen 处理 | `tauri::async_runtime::spawn` + `tokio::time::sleep` |
| 文件下载 | `tauri-plugin-http` (异步 reqwest) |
| 全局快捷键回调 | 同步闭包，重活 spawn 出去 |

### 现有规范 (参考 [src-tauri/src/lib.rs](src-tauri/src/lib.rs))
- 窗口 show 延迟 `WINDOW_SHOW_DELAY = 50ms` 防闪烁 — **不要随意改**，是已验证的视觉常量
- Linux 全屏后必须 `set_focus()` 修虚拟键盘焦点 bug
- Ubuntu 24.04/GNOME 需要 30ms 延迟再 `set_focus()` 修复装饰条 bug (#1122)

### 规则
- 异步函数 MUST NOT 阻塞；CPU 密集任务用 `tokio::task::spawn_blocking`
- 禁止跨 `.await` 持有 `std::sync::Mutex` guard，需要时改用 `tokio::sync::Mutex`
- 任何"睡几毫秒修 bug"的代码 MUST 写注释说明原因 + issue 编号

---

## §8 类型驱动设计 (Type-Driven Design)

### Pake 当前应用
- `PakeConfig` / `WindowConfig` (TS + Rust 双端) MUST 字段语义清晰，避免裸 `bool`/`String`
- 平台用 `#[cfg(target_os = "...")]` 编译期分支，**禁止运行时 if `cfg!(...)`** 来分平台

### 规则
- 新增配置字段 MUST 同时更新 TS 类型 ([bin/types.ts](bin/types.ts)) 与 Rust struct，并提供默认值
- 用 enum 表达互斥状态，不要用多个 bool

---

## §9 Pake 领域建模 (Domain Modeling)

| Pake 概念 | 实现 | 所有权含义 |
|----------|------|-----------|
| 一个被打包的应用 | `PakeAppOptions` (TS) → `PakeConfig` (Rust) | 配置值对象 |
| 一个运行时窗口 | `tauri::WebviewWindow` | Tauri 拥有，clone 廉价 |
| 多窗口集合 | `MultiWindowState` | App 拥有 |
| 注入脚本 | `&'static str` (include_str!) | 编译期资源 |
| 系统托盘/快捷键 | setup 期注册的 callback | 闭包拥有 handle clone |

### 边界
- TS 层 MUST 不感知 webview 内部细节（不直接操作 DOM 概念）
- Rust 层 MUST 不感知用户的 CLI 参数解析细节（只接受最终 PakeConfig）

---

## §10 Tauri / WebView 模式 (Tauri Patterns)

### 配置层
- 平台特定配置走 [tauri.macos.conf.json](src-tauri/tauri.macos.conf.json) / `tauri.windows.conf.json` / `tauri.linux.conf.json`
- 通用配置走 `tauri.conf.json`
- CLI 用户配置 → 在 [bin/helpers/tauriConfig.ts](bin/helpers/tauriConfig.ts) 合并到 `pake.json`，**禁止**直接改用户的 tauri.conf.json

### IPC (invoke)
- 所有 invoke handler 在 [src-tauri/src/app/invoke.rs](src-tauri/src/app/invoke.rs) 集中注册
- 命名 snake_case，与前端 `invoke('snake_case')` 对齐
- 参数/返回值 MUST 都是 `Serialize + Deserialize`

### Capabilities
- 任何新加的 plugin / API MUST 同步更新 [src-tauri/capabilities/default.json](src-tauri/capabilities/default.json)，遵循最小权限原则

---

## §11 注入策略 (Injection)

参考 [src-tauri/src/inject/](src-tauri/src/inject/)：`auth.js` / `component.js` / `custom.js` / `event.js` / `style.js` / `theme_refresh.js`

### 规则
- 注入脚本 MUST 用 IIFE 或 `{ }` 块作用域，防止污染目标网页 window
- 注入脚本 MUST 对目标网站做能力探测后再调用（防止破坏不存在的 API）
- 任何样式注入 MUST 加 Pake 专属前缀类名 (`pake-` / `__pake_`) 避免冲突
- 禁止注入脚本里硬编码用户站点的选择器（应通过配置传入或文档化）

---

## §12 跨平台规则 (Cross-Platform)

### 必须用 `#[cfg]` 隔离的领域
- 文件路径、应用数据目录
- 全屏 / 窗口装饰行为
- 系统托盘可见性
- 通知 API
- 菜单（macOS 才有顶部菜单）
- WebKit/WebView2 特定的 env var

### Linux 已知坑（必读）
- WebKitGTK 2.46+ 默认 DMA-BUF 渲染器在很多无独显或老 Mesa 环境下白屏
  → [src-tauri/src/lib.rs](src-tauri/src/lib.rs) 已默认 set `WEBKIT_DISABLE_DMABUF_RENDERER=1` / `WEBKIT_DISABLE_COMPOSITING_MODE=1`，**禁止删除**
- Wayland 下个别站点白屏，必要时 `GDK_BACKEND=x11` 兜底（不强制设置，给用户选择）
- bubblewrap 沙箱在 AppArmor 受限内核下会让 WebProcess 崩溃（需文档化）

### Windows 已知坑
- WebView2 运行时缺失 → 安装包 MUST 引导用户安装
- HiDPI 缩放需在 manifest 声明

### macOS 已知坑
- 顶部菜单 / Dock reopen 行为是必须 (§7 已示例)
- 签名 / 公证：CI ([release.yml](.github/workflows/release.yml)) 维护

---

## §13 性能与体积 (Performance & Size)

### 体积守则 (Pake 招牌)
- 任何新依赖 MUST 评估对最终二进制 / 安装包的影响
- 拒绝引入 >500KB 的纯 JS 运行时依赖
- Rust 依赖优先选 no_std 友好或 `default-features = false`
- [Cargo.toml](src-tauri/Cargo.toml) `[profile.release]` 已配 `opt-level = "z"` + `lto = "thin"` + `strip = true`，禁止改为 `opt-level = 3`

### 性能守则
- 启动时间是 KPI：setup 期禁止做网络 I/O / 大文件读
- 注入脚本 MUST 在 `document_start` 时机注入，不要等 `DOMContentLoaded`
- 用户感知的卡顿 → 优先用 spawn / 异步而不是同步阻塞

---

## §14 反模式 (Anti-Patterns) — Pake 专版

| 反模式 | 为什么对 Pake 不可接受 | 正确做法 |
|--------|------|---------|
| 在 invoke handler 里 `.unwrap()` | 用户机器一崩就是白屏/退出 | 返回 `Result` 让前端处理 |
| 加重型依赖（reqwest 全 feature 等） | 体积爆炸 | 启用必要 feature，或用 plugin-http |
| 在 setup() 里同步等网络 | 启动慢，丢失"快"卖点 | spawn 异步 |
| 注入脚本污染全局 window | 破坏目标网站 | IIFE / 块作用域 |
| 平台差异在 TS 层判断后传给 Rust | 重复逻辑、不一致 | Rust 用 `#[cfg]` 自决 |
| 直接 console.log 在 TS CLI 输出乱码 | 用户体验差 | 用 [bin/options/logger.ts](bin/options/logger.ts) |
| 改 `tauri.conf.json` 默认值改用户行为 | 破坏向后兼容 | 加 opt-in 字段 |

---

## §15 安全 (Security)

### Tauri allowlist / capabilities
- 最小权限：[capabilities/default.json](src-tauri/capabilities/default.json) 只开必要 API
- 禁止开 `shell:execute` 通配符权限
- HTTP 请求 MUST 限定域名 scope

### 注入与 XSS
- 注入到目标网页的内容 MUST 不来自不可信源
- 用户自定义 CSS/JS（高级用法）MUST 文档化风险

### 下载 & 文件
- [download_file](src-tauri/src/app/invoke.rs) MUST 校验目标路径在用户授权的下载目录内（防路径穿越）

### unsafe
- Pake 主代码库 MUST NOT 出现 `unsafe`；如必须，遵循 Rust Constitution §12

---

## §16 测试 (Testing)

### 现有测试结构
- TS 单元测试：[tests/unit/](tests/unit/) (vitest, [vitest.config.ts](vitest.config.ts))
- 集成测试：[tests/integration/](tests/integration/)
- Rust：目前依赖 `cargo check` + CI 多平台构建

### 规则
- 任何修改 [bin/builders/](bin/builders/) / [bin/helpers/](bin/helpers/) 的 PR MUST 补单元测试
- 修改窗口/快捷键/托盘行为 MUST 在 PR 描述中给出三平台手测结果
- CI ([quality-and-test.yml](.github/workflows/quality-and-test.yml)) 必须全绿才能合并

---

## §17 项目结构 (Project Layout)

```
Pake/
├── bin/                       # CLI (TypeScript)
│   ├── cli.ts                 # 入口
│   ├── builders/              # 平台 Builder (Mac/Win/Linux)
│   ├── helpers/               # 配置合并、Rust 调用、tauri 配置
│   ├── options/               # CLI 参数 / icon / logger
│   └── utils/                 # 通用工具
├── src-tauri/                 # 运行时 (Rust)
│   ├── src/
│   │   ├── main.rs            # 极简入口
│   │   ├── lib.rs             # run_app 主流程
│   │   ├── util.rs            # 配置加载
│   │   ├── app/               # window/menu/setup/invoke/config
│   │   └── inject/            # 注入到 webview 的 JS/CSS
│   ├── tauri.conf.json        # 通用 Tauri 配置
│   ├── tauri.{macos,windows,linux}.conf.json
│   ├── pake.json              # Pake 自定义配置
│   └── capabilities/          # Tauri 权限
├── tests/                     # vitest 单元 + 集成测试
├── docs/                      # 中英文用户文档
└── .github/workflows/         # CI: 构建/发布/质量
```

### 规则
- `main.rs` MUST 保持 ≤10 行（仅调用 `app_lib::run()`）
- 业务逻辑 MUST 在 `app/` 子模块中，不要堆 `lib.rs`
- 新增大特性 SHOULD 同时在 [docs/](docs/) 添加中英双语说明

---

## §18 提交与发布 (Workflow)

### 分支
- `main`: 永远可发布
- `pre-3.x`: 大版本预发支
- feature 分支从 `main` 切，PR 回 `main`

### Commit
- 遵循 Conventional Commits (`feat:` / `fix:` / `chore:` / `docs:` ...)
- 影响打包产物的 fix MUST 在 commit body 写明影响平台

### 发布
- 通过 [release.yml](.github/workflows/release.yml) 自动 tag 触发
- DO NOT 手动上传 release artifacts，全部走 CI

---

## §19 编译/常见错误索引

### Rust (src-tauri)
| 错误码 | 领域 | 首先检查 |
|--------|------|---------|
| E0382 | 所有权 | Tauri handle 是否在 spawn 前 clone？ |
| E0597 | 生命周期 | 是否应该 clone AppHandle 而非借用 Window？ |
| E0277 Send | 并发 | 跨 await 是否持锁？是否用了 `std::sync::Mutex`？ |
| `cfg!` vs `#[cfg]` 误用 | 跨平台 | 编译期分支必须用 `#[cfg]` 而非 `cfg!()` |

### TypeScript (bin/)
| 现象 | 检查 |
|------|------|
| `tauri build` 找不到 icon | [bin/options/icon.ts](bin/options/icon.ts) 平台后缀对了吗？ |
| 配置合并后字段丢失 | [bin/helpers/merge.ts](bin/helpers/merge.ts) 深合并是否覆盖了用户值？ |
| Linux 打包后白屏 | §12 Linux 已知坑 |

---

## §20 决策原则 (Decision Heuristics)

遇到争议时按优先级判断：

1. **用户体验**：能不能保持"一行命令出包、双击就用"？
2. **体积**：会让二进制 / 安装包变大吗？
3. **跨平台一致**：三个平台行为一致吗？
4. **安全**：是否扩大攻击面？
5. **可维护**：未来贡献者能看懂吗？

冲突时：用户体验 > 体积 > 一致性 > 安全（安全永远不可降到 0）> 可维护

---

*Sources: Rust Skills (m01-m15, domain-*) + Pake codebase audit*
*Companion: [rust.speckit.constitution](../rust.speckit.constitution) for general Rust rules*
