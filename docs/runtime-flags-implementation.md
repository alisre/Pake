# Runtime Flags 实现说明

## 功能概述

Pake 二进制在打包后支持两个运行时标志，无需重新编译：

| 标志                        | 功能                            |
| --------------------------- | ------------------------------- |
| `--url <http(s)://address>` | 覆盖构建时烧入的 URL            |
| `--ignore-cert`             | 忽略 TLS 证书错误（自签名证书） |
| `--help` / `-h`             | 打印用法并退出                  |

**使用示例：**

```bash
./myapp --url https://192.168.50.66 --ignore-cert
./myapp --url http://10.0.0.1:8080
./myapp --help
```

---

## 涉及文件

```
src-tauri/
  Cargo.toml                  ← 添加 wry feature + webkit2gtk Linux 依赖
  src/
    util.rs                   ← parse_runtime_url() / parse_runtime_ignore_cert()
    lib.rs                    ← 读取标志并注入配置；Linux TLS 信号绑定
    app/
      window.rs               ← effective_ignore_cert 逻辑（Linux 移除无效 Chromium 标志）
      config.rs               ← PakeConfig::ignore_certificate_errors 字段（已有）
```

---

## 各文件修改详情

### 1. `src-tauri/Cargo.toml`

两处修改：

```toml
# 1. tauri 添加 "wry" feature，才能调用 with_webview()
tauri = { version = "2.10.2", features = [
  "wry",          # ← 新增
  "tray-icon",
  "image-ico",
  "image-png",
  "macos-proxy",
] }

# 2. Linux-only 直接依赖 webkit2gtk（与 wry 内部版本一致：2.0.x）
[target.'cfg(target_os = "linux")'.dependencies]
webkit2gtk = "2"  # ← 新增整块
```

**原因：**

- `"wry"` feature 才能解锁 `WebviewWindow::with_webview()`
- `webkit2gtk = "2"` 提供 `WebViewExt`、`WebContextExt`、`connect_load_failed_with_tls_errors`、`allow_tls_certificate_for_host` 等 API

---

### 2. `src-tauri/src/util.rs`

新增两个函数：

```rust
/// 解析 --url <value>，返回 Some(url) 或 None。
/// 同时处理 --help / -h（打印用法并 exit(0)）。
pub fn parse_runtime_url() -> Option<String> {
    let args: Vec<String> = env::args().collect();

    // --help / -h
    if args.get(1).map(|a| a == "--help" || a == "-h").unwrap_or(false) {
        // 打印用法 …
        std::process::exit(0);
    }

    let mut iter = args.iter().skip(1);
    while let Some(arg) = iter.next() {
        if arg == "--url" {
            match iter.next() {
                Some(val) if !val.trim().is_empty() => return Some(val.clone()),
                _ => { eprintln!("[Pake] Warning: --url value is empty"); return None; }
            }
        }
    }
    None
}

/// 当进程参数包含 --ignore-cert 时返回 true。
pub fn parse_runtime_ignore_cert() -> bool {
    env::args().any(|a| a == "--ignore-cert")
}
```

---

### 3. `src-tauri/src/lib.rs`

**在 `run_app()` 入口处**（`get_pake_config()` 之后、`Builder::default()` 之前）读取标志并写入 config：

```rust
use util::{get_pake_config, parse_runtime_ignore_cert, parse_runtime_url};

// --url 覆盖 URL
if let Some(raw_url) = parse_runtime_url() {
    // 校验 scheme 必须是 http / https …
    if let Some(window) = pake_config.windows.first_mut() {
        window.url = raw_url;
    }
}

// --ignore-cert 设置 flag
if parse_runtime_ignore_cert() {
    if let Some(window) = pake_config.windows.first_mut() {
        window.ignore_certificate_errors = true;
    }
}
```

**在 `setup` 回调里，`set_window()` 之后**，为 Linux 绑定 WebKitGTK TLS 信号：

```rust
let window = set_window(app.app_handle(), &pake_config, &tauri_config)?;

#[cfg(target_os = "linux")]
{
    let needs_ignore_cert = pake_config
        .windows.first()
        .map(|w| w.ignore_certificate_errors || w.fullscreen)
        .unwrap_or(false);

    if needs_ignore_cert {
        window.with_webview(|webview| {
            use webkit2gtk::{WebContextExt, WebViewExt};
            let wkv = webview.inner();
            wkv.connect_load_failed_with_tls_errors(|view, failing_uri, cert, _flags| {
                if let Some(ctx) = view.context() {
                    let host = failing_uri
                        .strip_prefix("https://")
                        .and_then(|s| s.split('/').next())
                        .and_then(|s| s.split('@').last())
                        .unwrap_or("").to_owned();
                    if !host.is_empty() {
                        ctx.allow_tls_certificate_for_host(cert, &host);
                        view.load_uri(failing_uri);
                        return true;
                    }
                }
                false
            });
        }).ok();
    }
}
```

**关键设计：**

- `w.fullscreen == true`（kiosk 模式）时**自动**启用证书忽略，不需要额外传 `--ignore-cert`
- `with_webview` 必须在主线程执行（tauri 保证），信号回调也在主线程

---

### 4. `src-tauri/src/app/window.rs`

`effective_ignore_cert` 计算逻辑保持不变（Linux fullscreen 自动触发），但 **移除了对 Linux 无效的 Chromium 标志**：

```rust
// ✅ 保留（Windows/macOS 有效）
#[cfg(target_os = "windows")]
{
    windows_browser_args.push_str(" --ignore-certificate-errors");
}
#[cfg(target_os = "macos")]
{
    window_builder = window_builder.additional_browser_args("--ignore-certificate-errors");
}

// ❌ 已删除（对 WebKitGTK 无任何效果）
// #[cfg(target_os = "linux")]
// { linux_browser_args.push_str(" --ignore-certificate-errors"); }
```

**原因：** `--ignore-certificate-errors` 是 Chromium/Edge 专有 CLI 参数。WebKitGTK 使用完全不同的信号机制（见 lib.rs 中的处理）。

---

## 平台差异对比

| 平台                   | TLS 绕过机制                                                               | 触发条件                             |
| ---------------------- | -------------------------------------------------------------------------- | ------------------------------------ |
| **Linux** (WebKitGTK)  | `connect_load_failed_with_tls_errors` + `allow_tls_certificate_for_host()` | `--ignore-cert` 或 `fullscreen=true` |
| **Windows** (WebView2) | `--ignore-certificate-errors` Chromium 标志                                | `--ignore-cert`                      |
| **macOS** (WKWebView)  | `--ignore-certificate-errors` Chromium 标志                                | `--ignore-cert`                      |

> **注意：** Linux 的 kiosk/fullscreen 模式额外自动启用 TLS 绕过，因为部署场景通常是内网自签名证书的 kiosk 终端。

---

## 为什么不 fork wry

社区曾有一个 [alisre/wry fork (commit ea58d130)](https://github.com/alisre/wry/commit/ea58d130f62d28ac85a0215c599285affd01abb6) 将 TLS 绕过逻辑内嵌到 wry 本身，通过 `PlatformSpecificWebViewAttributes` 暴露新字段。

我们的方案通过 tauri 已有的 `with_webview()` 逃生舱门**绕过 wry 直接操作底层 webkit2gtk**，效果完全相同，且：

- 无需维护任何 wry fork
- 随 tauri/wry 正常升级无需 rebase
- 仅依赖 tauri 和 webkit2gtk 的公开稳定 API

---

## Ubuntu 性能优化记录

### 移除 `WEBKIT_DISABLE_COMPOSITING_MODE=1`（2026-05-09）

**修改文件：** `src-tauri/src/lib.rs`

**问题：** 历史代码在 Linux 启动时强制设置 `WEBKIT_DISABLE_COMPOSITING_MODE=1`，此环境变量会禁用 WebKitGTK 的 GPU 硬件合成，退化为 CPU 软件渲染所有合成层。

**影响：**

- CSS transform / opacity 动画 → CPU 纯软渲染
- 滚动性能差，帧率低
- 高 CPU 占用（尤其页面有动效时）

**修改前：**

```rust
if std::env::var("WEBKIT_DISABLE_COMPOSITING_MODE").is_err() {
    std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
}
```

**修改后：**

```rust
// GPU compositing: keep enabled (do NOT set WEBKIT_DISABLE_COMPOSITING_MODE).
// Ubuntu 24.04 + Mesa drivers support WebKitGTK hardware compositing correctly.
// Disabling it forces software rendering for all layer composition, which causes
// high CPU usage, janky scrolling, and poor CSS animation performance.
// Only set the env-var if the caller has explicitly opted out (e.g. for VM use).
```

**为何不彻底删除注释：** 在 VM 环境或老旧驱动（如 llvmpipe/softpipe）中，GPU 合成可能导致渲染异常。保留注释代码方便在这类环境中手动还原。

**保留 `WEBKIT_DISABLE_DMABUF_RENDERER=1` 的原因：**  
DMA-BUF 零拷贝纹理共享在部分 Mesa/Wayland 组合下仍存在视觉撕裂问题，kiosk 场景稳定性优先，仅带来轻微的 GPU 纹理传输开销（shm 路径 vs DMA-BUF）。

---

### 内存优化：减少 WebKitGTK 进程内存占用（2026-05-09）

**触发背景：** Ubuntu 系统日志显示 `__vm_enough_memory` 大量失败，`browser-binary` / `WebKitWebProces` / `WebKitNetworkPr` 不断尝试超大内存分配，最终导致物理内存耗尽 → `VM_FAULT_OOM` → GNOME Shell SIGBUS 崩溃。

#### 3.1 禁用 WebKitGTK 沙盒进程

**修改文件：** `src-tauri/src/lib.rs` — Linux 块新增

```rust
if std::env::var("WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS").is_err() {
    std::env::set_var("WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS", "1");
}
```

**原因：** WebKitGTK 默认额外启动一个特权 launcher 进程来孵化沙盒化的 WebProcess。kiosk 只访问固定内网 URL，渲染隔离安全需求低，禁用后合并进程层级，**节省约 30–60 MB RSS**。

调用者可在启动前设置同名环境变量为空字符串来覆盖此默认值（代码已用 `.is_err()` 保护）。

#### 3.2 限制 glibc malloc arena 数量

**修改文件：** `src-tauri/src/lib.rs` — Linux 块新增

```rust
if std::env::var("MALLOC_ARENA_MAX").is_err() {
    std::env::set_var("MALLOC_ARENA_MAX", "2");
}
```

**原因：** glibc 默认创建最多 `8 × CPU核数` 个 malloc arena，每个 arena 预映射约 64 MB 虚拟地址空间以减少锁争用。单标签 kiosk 不需要高并发分配，2 个 arena 完全够用，**减少约 64–256 MB 虚拟地址映射**，降低触发 `__vm_enough_memory` 拒绝的概率。

#### 3.3 禁用 WebKitGTK HTTP 缓存和页面缓存

**修改文件：** `src-tauri/src/lib.rs` — `with_webview()` 块内新增

```rust
use webkit2gtk::{CacheModel, WebContextExt, WebViewExt};
// ...
if let Some(ctx) = wkv.context() {
    ctx.set_cache_model(CacheModel::DocumentViewer);
}
```

**原因：** `CacheModel::DocumentViewer` 禁用：
- 磁盘 HTTP 缓存（通常缓存在 `~/.cache/` 下）
- 内存页面缓存（Back/Forward cache，默认保留最近几个页面的完整快照）

kiosk 只访问单一固定 URL，这两类缓存没有实际价值，**节省约 20–50 MB RSS**。

#### 3.4 合并 with_webview() 调用

原代码在 `needs_ignore_cert == true` 时才调用 `with_webview()`，新代码改为**始终调用一次** `with_webview()`，在其中同时完成：
1. 设置 `CacheModel::DocumentViewer`（无条件）
2. 绑定 TLS 信号（按需）

避免了两次调用的可能性，也确保缓存优化在所有 Linux 实例上生效。

**三项优化合计预期节省：约 50–110 MB RSS**

---

## 新分支上复现此功能的 Checklist

- [ ] `src-tauri/Cargo.toml`：`tauri` 添加 `"wry"` feature
- [ ] `src-tauri/Cargo.toml`：添加 `[target.'cfg(target_os = "linux")'.dependencies]` + `webkit2gtk = "2"`
- [ ] `src-tauri/src/util.rs`：添加 `parse_runtime_url()` 和 `parse_runtime_ignore_cert()`
- [ ] `src-tauri/src/lib.rs`：在入口读取两个标志并写入 `pake_config`
- [ ] `src-tauri/src/lib.rs`：Linux 块设置 `WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1` 和 `MALLOC_ARENA_MAX=2`
- [ ] `src-tauri/src/lib.rs`：`with_webview()` 内设置 `CacheModel::DocumentViewer` + 按需绑定 TLS 信号
- [ ] `src-tauri/src/app/window.rs`：确认已移除 Linux 下的 `--ignore-certificate-errors` browser arg
- [ ] `src-tauri/src/app/window.rs`：`effective_ignore_cert` Linux 条件包含 `|| window_config.fullscreen`
- [ ] `src-tauri/src/lib.rs`：**不要**强制设置 `WEBKIT_DISABLE_COMPOSITING_MODE=1`（保持 GPU 合成启用）
