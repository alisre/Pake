# Phase 1 — CLI Contract

本特性涉及两个面向用户的命令行接口：**已打包二进制的运行时参数**、**项目构建脚本**。

## 1. 已打包二进制运行时参数

### 1.1 `--url <URL>`

| 项 | 值 |
|---|---|
| 形式 | `--url <URL>`（必须紧跟一个值；不接受 `--url=<URL>` 形式 — 与现状一致） |
| 取值约束 | URL MUST 以 `http://` 或 `https://` 开头 |
| 行为 | 覆盖 `pake.json` 中编译期 URL；本次进程有效，不写盘 |
| 与位置参数 | 若同时给位置参数 URL，以 `--url` 为准 |
| 错误 1 | `--url` 缺少值 → `stderr: [browser] --url 参数缺少 URL 值` + 退出码 1 |
| 错误 2 | URL 不以 http/https 开头 → `stderr: [browser] --url 必须以 http:// 或 https:// 开头，收到: <值>` + 退出码 1 |

**示例**:

```bash
./browser --url https://stage.intra.example.com         # 切到预发
./browser --url https://localhost:8443 --ignore-cert    # 自签 HTTPS
./browser                                                # 用 pake.json 默认 URL
```

### 1.2 `--ignore-cert`

| 项 | 值 |
|---|---|
| 形式 | `--ignore-cert`（无值 flag） |
| 行为 | 启用 WebView 的 Chromium `--ignore-certificate-errors`，对所有 HTTPS 请求（含跳转）跳过证书校验 |
| 与配置 | 与 `pake.json.windows[0].ignore_certificate_errors` 是 **OR** 合并（任一为真即生效） |
| 默认 | 未指定 = 关闭（保持安全默认） |
| 范围 | 仅本次进程；不写盘 |

**移除项（破坏性变更声明）**: 旧原型代码中"`host==127.0.0.1` 或 `host==localhost` 时自动启用"的隐式行为**移除**。如需对 localhost 跳过证书，必须显式 `--ignore-cert` 或在 `pake.json` 中设置。

### 1.3 参数解析顺序

`main.rs` 在 `app_lib::run()` 之前线性扫描 `std::env::args().skip(1)`，遇到已识别的 flag 就 `set_var` 并继续；未识别参数原样保留供 Tauri / 后续逻辑使用。

## 2. 项目构建命令

### 2.1 `pnpm run build:ubuntu24`

| 项 | 值 |
|---|---|
| 入口 | [package.json](../../package.json) `scripts.build:ubuntu24` |
| 等价命令 | `PAKE_UBUNTU24=1 pake <url> --name <name> --targets deb` |
| 平台前置 | MUST 在 `process.platform === 'linux'` 下运行；其他平台输出友好错误并退出 1 |
| 产物 | `src-tauri/target/release/bundle/deb/<name>_<version>_amd64.deb`（**唯一**文件） |
| 副作用 | 设置环境变量 `PAKE_UBUNTU24=1`，触发 LinuxBuilder 锁定 `targets=['deb']` 与 deb depends 注入 |

**调用约定**:

```bash
# 直接调用
pnpm run build:ubuntu24 -- https://example.com --name my-app

# 等价
PAKE_UBUNTU24=1 node ./bin/cli.ts https://example.com --name my-app --targets deb
```

### 2.2 GitHub Actions workflow

| 项 | 值 |
|---|---|
| 文件 | [.github/workflows/build-ubuntu24.yml](../../.github/workflows/build-ubuntu24.yml) |
| 触发 | `workflow_dispatch`（手动）+ `push: tags: ['ubuntu24-v*']` |
| Runner | `ubuntu-24.04`（**禁止** `ubuntu-latest`） |
| Job 数 | 1 |
| 输入 | `inputs.url` (string, required)、`inputs.name` (string, default: `browser`) |
| 步骤 | checkout → setup-env (复用 [.github/actions/setup-env](../../.github/actions/setup-env)) → `pnpm install` → `pnpm run build:ubuntu24 -- "${{ inputs.url }}" --name "${{ inputs.name }}"` → `actions/upload-artifact@v4` 上传 `*_amd64.deb` |
| 产物名 | `<name>-ubuntu24-amd64-deb` |

## 3. 不变契约

- 既有 CLI 参数（`--name`/`--icon`/`--width`/...）行为**完全不变**
- 既有 `pake build` / `pake build:mac` / `pake build:win` / `pake build:linux` 脚本行为**完全不变**
- 已发布的 `pake.json` 字段**不**新增（仅复用既有 `ignore_certificate_errors`）

## 4. 验收脚本片段（供 quickstart 引用）

```bash
# 验收 1.1
./browser --url https://example.com   # 应加载 example.com

# 验收 1.2
./browser --url ftp://x               # 退出码 1，stderr 含"必须以 http:// 或 https:// 开头"

# 验收 1.3
./browser --url https://localhost:8443 --ignore-cert  # 自签证书页正常加载

# 验收 2.1
pnpm run build:ubuntu24 -- https://example.com --name testapp
ls src-tauri/target/release/bundle/deb/testapp_*_amd64.deb  # 应输出唯一一个文件
```
