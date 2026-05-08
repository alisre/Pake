# Tasks: Kiosk Mode Certificate Auto-Bypass & Runtime URL Override

**Feature Branch**: `002-kiosk-cert-url-param`  
**Input**: [spec.md](spec.md) — 2 user stories, 10 functional requirements  
**Tech Stack**: Rust (Tauri v2) · TypeScript (tests) · Markdown (docs)  
**Tests**: Unit tests included for URL parsing and kiosk heuristic logic

**Organization**: Tasks grouped by user story for independent implementation and testing.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no in-flight dependencies)
- **[US1]**: Runtime `--url` flag override
- **[US2]**: Kiosk mode automatic certificate bypass

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Establish test scaffolding and confirm existing tests pass before any changes are made.

- [X] T001 Verify existing test suite passes: run `pnpm test --run` and confirm zero failures — baseline for regression detection
- [X] T002 [P] Verify existing Rust build: run `cargo build --manifest-path src-tauri/Cargo.toml` and confirm clean compile — baseline for Rust changes

**Checkpoint**: Both builds pass with no failures before any code changes land.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Add the runtime args parsing function in `src-tauri/src/util.rs` — this is shared infrastructure required by both user stories. US1 uses it to extract `--url`; US2 reads the already-resolved `pake_config` and needs no additional foundational work.

- [X] T003 Add `parse_runtime_args()` function to `src-tauri/src/util.rs` that:
  - Reads `std::env::args()` and extracts the `--url <value>` pair
  - Returns `Option<String>` (None when flag is absent or value is empty/whitespace)
  - Emits a `[Pake] Warning: --url value is empty, using baked-in URL` to stderr when value is present but blank
  - Does NOT validate URL scheme or format (validation happens in US1 implementation tasks)

**Checkpoint**: `parse_runtime_args()` compiles; foundational work unblocks both user story phases.

---

## Phase 3: User Story 1 — Runtime `--url` Flag (Priority: P1) 🎯 MVP

**Goal**: A compiled Pake binary accepts `--url <address>` at launch time and navigates to that URL instead of the baked-in one. Backward compatible: omitting the flag loads the original URL.

**Independent Test**: Build the default `src-tauri` binary. Run `./target/debug/pake-app --url https://example.org`. Confirm the webview loads `https://example.org` not the URL in `pake.json`.

### Tests for User Story 1

- [X] T004 [P] [US1] Add unit test in `tests/unit/runtime-url-arg.test.ts` (or `.rs` integration test in `src-tauri/src/`) covering:
  - `--url https://valid.example.com` → returns the URL string
  - `--url ""` (empty) → returns None + warning emitted
  - `--url ftp://bad-scheme.com` → parse succeeds at this layer (scheme rejection is in T007)
  - No `--url` flag → returns None
  - `--url` present without value (malformed argv) → treated as empty → None + warning

### Implementation for User Story 1

- [X] T005 [US1] Integrate `parse_runtime_args()` in `src-tauri/src/lib.rs` — call it immediately after `get_pake_config()` returns; store result as `runtime_url: Option<String>`

- [X] T006 [US1] Add URL scheme validation in `src-tauri/src/lib.rs` (or a helper in `util.rs`):
  - If `runtime_url` is `Some(s)` and `s` is non-empty:
    - Parse with `url::Url::parse(&s)` (the `url` crate is already a transitive dependency via Tauri)
    - If parse fails → `eprintln!("[Pake] Error: --url '{}' is not a valid URL", s)` + `std::process::exit(1)`
    - If scheme is not `http` or `https` → `eprintln!("[Pake] Error: --url only accepts http:// or https:// schemes; url_type is immutable after build")` + `std::process::exit(1)`
  - Print info log to stdout: `println!("[Pake] URL overridden at runtime: {}", s)` when override is active

- [X] T007 [US1] Override the URL in `pake_config` before it is used in `src-tauri/src/lib.rs`:
  - After validation passes, mutate `pake_config.windows[0].url = validated_url_string`
  - Keep `pake_config.windows[0].url_type` unchanged (must remain `"web"`)
  - All subsequent code that reads `pake_config` (window construction, `MultiWindowState`) automatically picks up the overridden URL — no further changes needed to `app/window.rs`

- [X] T008 [P] [US1] Add `--url` as a discoverable help entry by updating the binary's argument hint: in `src-tauri/src/util.rs` add a `--help` handler that prints `Usage: <app> [--url <http(s)://address>]` when `--help` or `-h` is passed as the first argument, then exits 0. This ensures `SC-005` (discoverability) is met.

**Checkpoint**: US1 complete when `./binary --url https://foo.com` navigates to `https://foo.com`, `./binary` (no flag) navigates to baked-in URL, and `./binary --url bad` exits with code 1.

---

## Phase 4: User Story 2 — Kiosk Mode Cert Auto-Bypass on Linux (Priority: P2)

**Goal**: On Linux, when `fullscreen: true` in `pake.json`, the WebKitGTK WebView automatically receives `--ignore-certificate-errors` without any operator rebuild flag. Non-kiosk mode, macOS, and Windows are unaffected.

**Independent Test**: Build binary with `fullscreen: true` in `src-tauri/pake.json`. On Ubuntu 24.04, run the binary against an HTTPS endpoint with a self-signed certificate. Confirm the page loads without a TLS error interstitial.

### Tests for User Story 2

- [X] T009 [P] [US2] Add unit test in `tests/unit/kiosk-cert-bypass.test.ts` covering the heuristic logic (can mock the window config):
  - `fullscreen: true` + `ignore_certificate_errors: false` (default) on Linux → effective cert bypass = true
  - `fullscreen: false` + `ignore_certificate_errors: false` on Linux → effective cert bypass = false
  - `fullscreen: true` + `ignore_certificate_errors: true` (explicit) → cert bypass = true (explicit opt-in unchanged)
  - `fullscreen: true` on macOS or Windows → cert bypass controlled only by `ignore_certificate_errors` flag (no auto-bypass)

### Implementation for User Story 2

- [X] T010 [US2] Modify the Linux cert-bypass block in `src-tauri/src/app/window.rs` (around line 304, inside the `if window_config.ignore_certificate_errors` block):

  Current code:
  ```rust
  if window_config.ignore_certificate_errors {
      #[cfg(target_os = "linux")]
      { linux_browser_args.push_str(" --ignore-certificate-errors"); }
      ...
  }
  ```

  New logic: compute effective bypass flag before the block, then apply it:
  ```rust
  #[cfg(target_os = "linux")]
  let effective_ignore_cert = window_config.ignore_certificate_errors
      || window_config.fullscreen;  // kiosk auto-bypass

  #[cfg(not(target_os = "linux"))]
  let effective_ignore_cert = window_config.ignore_certificate_errors;

  if effective_ignore_cert {
      #[cfg(target_os = "linux")]
      { linux_browser_args.push_str(" --ignore-certificate-errors"); }
      #[cfg(target_os = "windows")]
      { windows_browser_args.push_str(" --ignore-certificate-errors"); }
      #[cfg(target_os = "macos")]
      { window_builder = window_builder.additional_browser_args("--ignore-certificate-errors"); }
  }
  ```

  Note: macOS and Windows bypass path is only reached when `ignore_certificate_errors` is explicitly `true` (not via fullscreen), so their behaviour is unchanged.

**Checkpoint**: US2 complete when a fullscreen binary on Ubuntu 24.04 loads a self-signed-cert HTTPS page without error, and a non-fullscreen binary on the same machine still shows TLS errors.

---

## Phase 5: Polish & Cross-Cutting Concerns

**Purpose**: Documentation updates and final validation across platforms.

- [X] T011 [P] Update `docs/cli-usage.md` — add a "Runtime Flags (Binary Execution)" section after the existing CLI build flags section, documenting:
  - `--url <http(s)://address>` — Override the baked-in URL at launch time (no rebuild required)
  - Kiosk certificate auto-bypass: note that binaries built with `fullscreen: true` automatically bypass self-signed certificate errors on Linux (Ubuntu 24.04 verified); security implication callout required

- [X] T012 [P] Update `docs/cli-usage_CN.md` — same content as T011 translated to Chinese

- [X] T013 Run full test suite and confirm no regressions: `pnpm test --run && cargo test --manifest-path src-tauri/Cargo.toml`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 completion — provides `parse_runtime_args()` used by US1
- **Phase 3 (US1)**: Depends on Phase 2 — T004 (test) can be written in parallel with T003
- **Phase 4 (US2)**: Depends on Phase 2 — can proceed in parallel with Phase 3 after Phase 2 completes
- **Phase 5 (Polish)**: Depends on Phase 3 + Phase 4 completion

### User Story Dependencies

- **US1 (P1)**: Depends on T003 (foundational `parse_runtime_args`). T004 test can be written before T005-T008.
- **US2 (P2)**: Does NOT depend on US1. Can start immediately after Phase 2 completes. Only touches `src-tauri/src/app/window.rs` — no overlap with US1 files.

### Parallel Opportunities

After Phase 2 (T003) completes, the following can run in parallel:

```
T003 complete
     ├── US1: T004 (test) ─── T005 ─── T006 ─── T007 ─── T008
     └── US2: T009 (test) ─── T010
         (both complete)
              └── T011 [P] ─── T012 [P] ─── T013
```

---

## Implementation Strategy

**MVP**: Complete Phase 3 (US1) first — the `--url` runtime flag is P1 and delivers standalone kiosk fleet management value without any dependency on US2.

**Increment 2**: Complete Phase 4 (US2) — the certificate bypass is P2 and is a pure Rust change in `window.rs` isolated from US1.

**Delivery**: After both stories, run Phase 5 to ship documentation.

---

## Summary

| Metric | Value |
|---|---|
| Total tasks | 13 |
| US1 tasks | 5 (T004–T008) |
| US2 tasks | 2 (T009–T010) |
| Setup / Foundational | 3 (T001–T003) |
| Polish / Docs | 3 (T011–T013) |
| Parallelizable tasks | 8 marked [P] |
| Primary files changed | `src-tauri/src/util.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/app/window.rs`, `docs/cli-usage.md`, `docs/cli-usage_CN.md` |
| New test files | `tests/unit/runtime-url-arg.test.ts`, `tests/unit/kiosk-cert-bypass.test.ts` |
| Suggested MVP | Phase 1 + Phase 2 + Phase 3 (US1 only) |
