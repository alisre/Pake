# Phase 1 — Data Model

本特性无新数据库 / 持久化模型。仅涉及**内存配置对象**与**进程级环境变量**两类状态。

## 1. 内存数据：`PakeConfig.windows[0]`（既有，扩展使用）

定义位置：[src-tauri/src/app/config.rs](../../src-tauri/src/app/config.rs)

| 字段 | 类型 | 来源 | 本特性影响 |
|---|---|---|---|
| `url` | `String` | `pake.json` 编译期值 | 启动期可被 `PAKE_RUNTIME_URL` 覆盖 |
| `url_type` | `String` (`"web"`/`"local"`) | `pake.json` | 当 `url` 被覆盖时强制设为 `"web"` |
| `ignore_certificate_errors` | `bool` | `pake.json` | 与 `PAKE_IGNORE_CERT` env OR 合并 |
| 其他字段 | — | — | **不动** |

**变更点**: 仅 [src-tauri/src/util.rs](../../src-tauri/src/util.rs)::`get_pake_config()` 在 `serde_json::from_str` 之后、返回之前进行覆盖。其他读取方均不感知。

**不变量**:
- 覆盖只发生在进程启动期（首次调用 `get_pake_config`），后续读取得到一致值
- 不写回 `pake.json`（持久化是反需求）

## 2. 进程级环境变量（新增）

由 `main.rs` 解析 CLI 后写入，由 `util.rs` 读取消费。

| 变量名 | 写入方 | 读取方 | 取值 | 缺省 |
|---|---|---|---|---|
| `PAKE_RUNTIME_URL` | `main.rs`（`--url <URL>` 命中时） | `util.rs::get_pake_config` | `http://*` 或 `https://*` 字符串 | 未设置 |
| `PAKE_IGNORE_CERT` | `main.rs`（`--ignore-cert` 命中时） | `util.rs::get_pake_config` | **任意非空值**视为启用（约定写入 `"1"`） | 未设置 |

**安全约束**:
- 这两个变量**仅由本进程自己写**，不暴露给 webview 内 JS、不传给子进程
- `main.rs` 在 `app_lib::run()` 调用前完成所有写入，避免竞态

## 3. TS Builder 内部状态（新增）

| 字段 | 位置 | 类型 | 作用 |
|---|---|---|---|
| `ubuntu24Only` | [bin/builders/LinuxBuilder.ts](../../bin/builders/LinuxBuilder.ts) 实例字段 | `boolean` | 由 `process.env.PAKE_UBUNTU24 === '1'` 计算；为 true 时强制 `targets=['deb']`、`buildArch='x64'`、合并 deb depends |

**变更点**: `bin/helpers/tauriConfig.ts` 在合并 `tauri.linux.conf.json` 时，若 `ubuntu24Only` 为 true，注入：

```jsonc
{
  "bundle": {
    "active": true,
    "targets": ["deb"],
    "linux": {
      "deb": {
        "depends": [
          "libwebkit2gtk-4.1-0",
          "libgtk-3-0",
          "libayatana-appindicator3-1",
          "curl",
          "wget"
        ]
      }
    }
  }
}
```

## 4. 状态转换图（运行时）

```text
[二进制启动]
     │
     ▼
main.rs 扫描 argv
     │
     ├── 命中 --url <URL>     → set_env PAKE_RUNTIME_URL
     ├── 命中 --ignore-cert    → set_env PAKE_IGNORE_CERT=1
     └── 未命中                → 不写
     │
     ▼
app_lib::run()
     │
     ▼
util.rs::get_pake_config()
     │
     ├── 读 pake.json → PakeConfig
     ├── 若 PAKE_RUNTIME_URL 存在 → 覆盖 windows[0].url / url_type="web"
     └── 若 PAKE_IGNORE_CERT 存在 → windows[0].ignore_certificate_errors |= true
     │
     ▼
WindowBuilder（window.rs）
     │
     └── ignore_certificate_errors 为 true → 三平台分别注入 --ignore-certificate-errors
```

## 5. 不在范围

- 不引入新的 invoke 命令
- 不改 `capabilities/default.json`
- 不持久化任何状态到磁盘
- 不引入新的 Rust crate（特别是不引入 `chrono`、`clap`）
