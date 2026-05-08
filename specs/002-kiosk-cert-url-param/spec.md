# Feature Specification: Kiosk Mode Certificate Auto-Bypass & Runtime URL Override

**Feature Branch**: `002-kiosk-cert-url-param`  
**Created**: 2026-05-08  
**Status**: Draft  
**Input**: User description: "实现在ubuntu 24.04 Desktop 下运行二进制程序 kiosk模式时，能自动忽略自签名证书错误提示；增加--url 参数，可以打开指定的web 地址"

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Runtime URL Override via `--url` Flag (Priority: P1)

A kiosk operator deploys the same Pake binary on multiple Ubuntu 24.04 Desktop machines, each serving a different internal web application. Instead of building a separate binary for each URL, the operator passes `--url <address>` at launch time so one binary serves any target endpoint.

**Why this priority**: Without this capability, every URL change requires a full rebuild and redeployment of the binary — impractical for kiosk fleet management. A runtime flag is the highest-impact, most deployment-friendly change.

**Independent Test**: Launch the compiled Pake binary with `./myapp --url https://192.168.1.100` and confirm the webview opens that URL instead of the URL baked in at build time. No rebuild required.

**Acceptance Scenarios**:

1. **Given** a compiled Pake binary with a baked-in URL (e.g., `https://example.com`), **When** the binary is launched with `--url https://192.168.1.100/dashboard`, **Then** the webview loads `https://192.168.1.100/dashboard` and the baked-in URL is not loaded.
2. **Given** a compiled Pake binary launched without the `--url` flag, **When** the binary starts normally, **Then** the webview loads the original baked-in URL from `pake.json` (backward-compatible behaviour).
3. **Given** a binary launched with `--url` pointing to an invalid or malformed URL, **When** the binary starts, **Then** the app logs an error message and exits with a non-zero exit code; the window does not open.
4. **Given** a binary launched with `--url https://192.168.1.100`, **When** the window renders, **Then** internal navigation rules (e.g., `force_internal_navigation`, `internal_url_regex`) from `pake.json` still apply as configured at build time.

---

### User Story 2 — Automatic Certificate Error Bypass in Kiosk Mode on Ubuntu 24.04 (Priority: P2)

An enterprise deploys Pake-packaged apps on Ubuntu 24.04 Desktop kiosk terminals that connect to internal HTTPS services using self-signed certificates. Currently, the WebKitGTK-based WebView displays a TLS error page that prevents the kiosk from loading. The operator wants the binary — when running in fullscreen/kiosk mode — to automatically bypass self-signed certificate errors without requiring a separate rebuild flag.

**Why this priority**: Certificate errors block kiosk operation entirely, but the workaround (`--ignore-certificate-errors` CLI build flag) requires a rebuild. Automating this per-mode reduces deployment friction and is the primary pain point for kiosk deployments.

**Independent Test**: Build a Pake binary with `fullscreen: true` in `pake.json` targeting a URL served with a self-signed certificate on Ubuntu 24.04. Launch the binary. Confirm the page loads without a TLS error page — no extra build flags required.

**Acceptance Scenarios**:

1. **Given** a Pake binary built with `fullscreen: true` and running on Ubuntu 24.04, **When** the binary launches and connects to an HTTPS endpoint using a self-signed certificate, **Then** the page loads successfully without a TLS error interstitial.
2. **Given** a Pake binary built with `fullscreen: false` (non-kiosk) and running on Ubuntu 24.04, **When** the binary connects to the same self-signed HTTPS endpoint, **Then** the TLS error interstitial is shown normally (existing behaviour is preserved for non-kiosk mode).
3. **Given** a binary where `ignore_certificate_errors: true` is already explicitly set in `pake.json`, **When** the binary runs in any mode, **Then** the explicit flag still takes effect (auto-bypass should not override explicit opt-in).
4. **Given** a Pake binary with `fullscreen: true` running on macOS or Windows, **When** it connects to a self-signed HTTPS endpoint, **Then** behaviour is unchanged from today (auto-bypass is Ubuntu/Linux-specific).

---

### Edge Cases

- What happens when `--url` is supplied AND `--url` value conflicts with `force_internal_navigation`? The navigation rules from `pake.json` remain in effect; the `--url` override only sets the initial page, not navigation policies.
- What happens when the `--url` value uses `http://` on a binary built for HTTPS? The URL is accepted as-is; no protocol upgrading is performed.
- What if `--url` is provided but the value is empty (`--url ""`)? The binary treats this as a missing value and falls back to the baked-in URL, logging a warning.
- On Ubuntu 24.04, if the binary is built with both `fullscreen: true` and `ignore_certificate_errors: false` explicitly in `pake.json`, which takes precedence? The auto-bypass from kiosk mode applies; explicit `false` only suppresses auto-bypass if `ignore_certificate_errors` is NOT the default (see Assumptions).
- What happens when the runtime `--url` points to a `local` file path while the binary was built with `url_type: "web"`? This is rejected with a clear error (`[Pake] Error: --url only accepts http:// or https:// schemes; url_type is immutable after build`); `url_type` remains immutable after build.
- What if `--url` is an empty string? Falls back to baked-in URL with a stderr warning; does not exit with error (distinguished from syntactically invalid URLs).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The compiled Pake binary MUST accept an optional `--url <string>` command-line argument at runtime. Only `http://` and `https://` URL schemes are accepted; other schemes (e.g., `file://`) MUST be rejected with a clear error message. No URL whitelist is enforced.
- **FR-002**: When `--url` is provided, the webview MUST navigate to that URL instead of the URL embedded in `pake.json` at build time.
- **FR-003**: When `--url` is omitted, the binary MUST behave identically to today's behaviour (load the baked-in URL from `pake.json`).
- **FR-004**: When `--url` is a non-empty but syntactically invalid URL (fails URL parsing), the binary MUST exit with a descriptive error message and exit code 1. When `--url` is an empty string or whitespace-only, the binary MUST fall back to the baked-in URL and emit a warning to stderr.
- **FR-005**: All window configuration (dimensions, title bar style, navigation rules, zoom, etc.) from `pake.json` MUST remain in effect regardless of whether `--url` is used.
- **FR-006**: On Linux, when `fullscreen: true` is configured in `pake.json`, the runtime MUST automatically apply certificate-error bypass equivalent to `--ignore-certificate-errors` for the WebKitGTK WebView. No separate `kiosk_mode` field is introduced; `fullscreen: true` is the sole trigger.
- **FR-007**: The auto-bypass behaviour described in FR-006 MUST be limited to Linux targets only; macOS and Windows behaviour MUST remain unchanged.
- **FR-008**: When `ignore_certificate_errors: true` is explicitly set in `pake.json`, the certificate bypass MUST apply regardless of fullscreen mode (existing behaviour preserved).
- **FR-009**: The `--url` runtime argument MUST be documented in `docs/cli-usage.md` and `docs/cli-usage_CN.md` as a runtime (binary) flag — distinct from build-time CLI flags.
- **FR-010**: When `--url` overrides the baked-in URL, the binary MUST emit exactly one info-level log line to stdout on startup: `[Pake] URL overridden at runtime: <url>`.

### Key Entities

- **PakeConfig / WindowConfig** (Rust): The in-memory config struct loaded from `pake.json` at startup; `url` field in `WindowConfig` must be overridable by the runtime `--url` arg before window construction begins.
- **Runtime Arguments** (Rust): A new parsing step in `util.rs` or `lib.rs` that reads `std::env::args()` to extract `--url` before `get_pake_config()` is used for window construction.
- **Kiosk Mode Heuristic** (Rust/Linux): The logic in `app/window.rs` that determines whether to auto-append `--ignore-certificate-errors` based on `fullscreen` flag on Linux.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On Ubuntu 24.04 Desktop, a Pake binary built with `fullscreen: true` successfully loads an HTTPS page served with a self-signed certificate without any user interaction — zero TLS error interstitials observed.
- **SC-002**: A single compiled binary can be redirected to any valid HTTPS URL via the `--url` flag with no rebuild required — verified across at least two distinct URLs on the same binary.
- **SC-003**: Non-kiosk binaries (fullscreen: false) on Ubuntu 24.04 show TLS errors normally — existing behaviour regression rate is 0%.
- **SC-004**: Binaries built for macOS and Windows are unaffected by this change — existing test suite passes at 100% on those platforms.
- **SC-005**: The `--url` flag is discoverable: operators can run `./myapp --help` and see the flag listed (or run `./myapp --url` without a value and receive a clear usage message).

## Clarifications

### Session 2026-05-08

- Q: Should the `--url` runtime flag enforce any restriction on allowed target URLs (e.g., domain whitelist)? → A: No restrictions — any valid URL is accepted; access control is the operator's responsibility.
- Q: Should certificate auto-bypass be coupled to `fullscreen: true` or require a separate `kiosk_mode` field? → A: Keep coupled — `fullscreen: true` triggers auto certificate bypass on Linux; no new field introduced (YAGNI).
- Q: What is the exact boundary between "empty `--url`" (fallback) and "invalid `--url`" (exit with error)? → A: Empty string or whitespace-only value falls back to baked-in URL with a stderr warning; any non-empty but syntactically invalid URL (fails URL parse) causes immediate exit with error code 1.
- Q: Should `--url file://...` or local relative paths be accepted as runtime overrides? → A: No — `--url` only accepts `http://` or `https://` schemes; any other scheme is rejected with a clear error message referencing that `url_type` is immutable after build.
- Q: What observability should the `--url` override produce at runtime? → A: A single info-level log line on startup when `--url` is active: `[Pake] URL overridden at runtime: <url>`; no additional logging required.

## Assumptions

- The target platform for the kiosk auto-bypass is Ubuntu 24.04 Desktop (x86_64); ARM variants of Ubuntu Linux are out of scope for this feature but the code change is expected to be platform-generic within Linux.
- "Kiosk mode" is operationally defined as a binary built with `fullscreen: true` in `pake.json`; no separate dedicated `kiosk_mode` field is introduced (YAGNI). This coupling is intentional: all kiosk deployments are expected to be fullscreen.
- The `--url` runtime flag overrides only the initial navigation URL; it does NOT override `url_type` (must remain `"web"` for remote URLs) or any other `pake.json` configuration. Only `http://` and `https://` schemes are accepted at runtime.
- Security implication of certificate bypass is accepted by the operator: self-signed certificate auto-bypass is an intentional kiosk deployment choice, not a general security weakening. No URL whitelist is enforced — access control is entirely the operator's responsibility. Documentation must call this out explicitly.
- Pake binaries on Linux use WebKitGTK (via Tauri v2); the `--ignore-certificate-errors` browser arg accepted by the WebKit process is the mechanism for the bypass.
- Existing unit and integration tests must continue to pass after implementation; new tests must be added for `--url` arg parsing and the kiosk-cert-bypass heuristic.
- The `--url` flag is a runtime-only argument; it is not visible or relevant during the `pake <url>` build-time CLI invocation.
- Mobile platforms are out of scope.
