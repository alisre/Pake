# Phase 1 — Quickstart

> 本文档面向**实现完成后**的验证流程；当前 plan 阶段仅作为规约。

## 0. 前置环境

- macOS / Linux 开发机均可
- 验收 P2（Ubuntu 24.04 deb 构建）需要：
  - 真机/容器/GHA runner 跑 `ubuntu-24.04`（不能用 22.04 / latest）
  - 已安装 `webkit2gtk-4.1`、`libgtk-3-dev`、`libayatana-appindicator3-dev`、`librsvg2-dev`、`build-essential`、`curl`、`wget`、`file`
- Node 20+、pnpm 9+、Rust 1.85+

## 1. 拉取与切分支

```bash
git clone https://github.com/tw93/Pake.git
cd Pake
git checkout 001-cli-url-ssl-ubuntu
pnpm install
```

## 2. 构建 Ubuntu 24.04 专属 deb（验证 User Story 3 / P2）

### 2.1 本地（Ubuntu 24.04 主机或容器）

```bash
pnpm run build:ubuntu24 -- https://example.com --name browser
```

预期：
- 终端输出无 `cargo` 解析错误（验证 FR-009 已修复）
- 仅产出 1 个 `.deb` 文件，路径形如：

```text
src-tauri/target/release/bundle/deb/browser_3.11.3_amd64.deb
```

- 无 `appimage/` `rpm/` 子目录产物

### 2.2 检查 deb 元数据

```bash
dpkg -I src-tauri/target/release/bundle/deb/browser_*_amd64.deb
```

预期：
- `Architecture: amd64`
- `Depends:` 字段包含 `libwebkit2gtk-4.1-0`、`libgtk-3-0`、`libayatana-appindicator3-1`、`curl`、`wget`

### 2.3 干净 Ubuntu 24.04 容器安装验证

```bash
docker run --rm -it -v "$PWD":/work -w /work ubuntu:24.04 bash -lc '
  apt-get update -y && apt-get install -y ./src-tauri/target/release/bundle/deb/browser_*_amd64.deb
  which browser && browser --version || echo "binary present"
'
```

预期：`apt-get install` 成功，无依赖缺失。

### 2.4 GHA 跑通

手动触发 [.github/workflows/build-ubuntu24.yml](../../.github/workflows/build-ubuntu24.yml)：

- 输入 `url=https://example.com`、`name=browser`
- 工作流 1 个 job、跑在 `ubuntu-24.04`
- artifact 命名 `browser-ubuntu24-amd64-deb`，内含唯一 `.deb` 文件

## 3. 运行时 `--url` 覆盖（验证 User Story 1 / P1）

在已安装 `browser` 的 Ubuntu 24.04 上：

```bash
# 场景 1：覆盖到 stage
browser --url https://stage.intra.example.com
# 预期：窗口加载 stage.intra.example.com

# 场景 2：无参 → 默认
browser
# 预期：加载 pake.json 内编译期 URL

# 场景 3：非法协议
browser --url ftp://foo
echo "exit=$?"
# 预期：stderr 提示协议错误，exit=1
```

## 4. 自定义证书 SSL 容忍（验证 User Story 2 / P1）

### 4.1 本地起自签 HTTPS 服务

```bash
# 任选其一
mkcert -install
mkcert localhost 127.0.0.1
python3 -m http.server 8443 --bind 127.0.0.1   # 配合反向代理或换用 https.server 脚本
# 或用 openssl 自签 + nginx
```

### 4.2 验证

```bash
# 场景 1：开关开启 → 应正常加载
browser --url https://localhost:8443 --ignore-cert
# 预期：页面正常渲染

# 场景 2：开关关闭 → 应被 SSL 错误页拦截（默认安全）
browser --url https://localhost:8443
# 预期：WebView 显示证书错误页 / 不加载

# 场景 3：构建期开启
# 编辑 src-tauri/pake.json，把 windows[0].ignore_certificate_errors 设为 true
pnpm run build:ubuntu24 -- https://localhost:8443 --name browser-trust-all
# 预期：browser-trust-all 不带任何 flag 也能加载自签站点
```

### 4.3 回归

```bash
browser --url https://www.example.com           # 公网正常证书 → 仍正常
browser --url https://expired.badssl.com        # 默认应该被拦
browser --url https://expired.badssl.com --ignore-cert  # 显式跳过
```

## 5. 跨平台运行时回归（守 FR-008）

在 macOS / Windows 本地构建路径上：

```bash
pnpm run build:mac    # 应与本特性合并前行为一致
pnpm run build:win    # 应与本特性合并前行为一致
pnpm run build        # 自动识别平台，行为不变
```

预期：均成功；产物位置、文件名、依赖等无变化。

## 6. 单元测试

```bash
# Rust 端
cd src-tauri && cargo test --lib

# TS 端
pnpm test -- linux-builder-ubuntu24
```

预期：全部通过。

## 7. 体积/性能基线（守 SC-005）

```bash
# 与基线 commit 对比
ls -la src-tauri/target/release/bundle/deb/*.deb
# deb 大小应在基线 ±5% 内
```

## 8. 失败排查

| 现象 | 排查 |
|---|---|
| `cargo` 编译报 `expected one of`/`unclosed delimiter` | 检查 [src-tauri/Cargo.toml](../../src-tauri/Cargo.toml) 是否每个依赖独占一行；检查 [src-tauri/src/lib.rs](../../src-tauri/src/lib.rs) `.on_window_event(...)` 是否完整 |
| deb 在 22.04 装不上 | 预期行为；本特性仅承诺 Ubuntu 24.04 |
| `--url` 不生效 | 确认未把 `--url` 拼成 `--url=...`；当前实现只接受空格分隔 |
| `--ignore-cert` 在 macOS 上无效 | 检查 [src-tauri/src/app/window.rs:292](../../src-tauri/src/app/window.rs) 中 `additional_browser_args` 是否被构建期 `#[cfg(target_os="macos")]` 编入 |
