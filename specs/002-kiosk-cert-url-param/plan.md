# Implementation Plan: Kiosk Mode Certificate Auto-Bypass & Runtime URL Override

**Branch**: `002-kiosk-cert-url-param` | **Date**: 2026-05-08 | **Spec**: [spec.md](spec.md)  
**Input**: Feature specification from `specs/002-kiosk-cert-url-param/spec.md`  
**Guiding constraint**: Keep code structure as simple and maintainable as possible (YAGNI, minimal diff)

## Summary

Two focused runtime changes to the compiled Pake binary:

1. **`--url` runtime flag** — `src-tauri/src/util.rs` + `lib.rs`: parse `std::env::args()` for `--url <value>`, validate scheme (`http`/`https` only), mutate `pake_config.windows[0].url` before window construction. Zero new Cargo dependencies.

2. **Kiosk cert auto-bypass** — `src-tauri/src/app/window.rs`: on Linux, extend the existing `if window_config.ignore_certificate_errors` condition to also trigger when `window_config.fullscreen` is `true`. A single `||` operator addition.

Both changes are additive, backward-compatible, and touch ≤ 3 files total.

## Technical Context

**Language/Version**: Rust stable (pinned in `rust-toolchain.toml`) + TypeScript 5.x (tests)  
**Primary Dependencies**: Tauri v2 (existing); `url` crate via Tauri transitive dependency (no new entry in `Cargo.toml`)  
**Storage**: N/A — no persistent storage changes  
**Testing**: Vitest (TypeScript unit tests); `cargo test` (Rust)  
**Target Platform**: Linux Ubuntu 24.04 Desktop x86_64 (cert bypass); all platforms (`--url` flag)  
**Project Type**: Desktop app runtime (Rust/Tauri v2) + build CLI (TypeScript/Node.js)  
**Performance Goals**: Args parsing + URL validation is synchronous O(1); no startup latency impact  
**Constraints**: No new Cargo crates; no changes to `pake.json` schema; no changes to build-time CLI (`bin/`)  
**Scale/Scope**: Minimal diff — 2 source files changed, 1 helper function added, 1 condition modified

## Constitution Check

_GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design — all gates still pass._

| Principle                               | Status  | Notes                                                                                          |
| --------------------------------------- | ------- | ---------------------------------------------------------------------------------------------- |
| I. Lightweight-First — no Electron      | ✅ PASS | No new dependencies of any kind                                                                |
| II. Single-Command Experience           | ✅ PASS | `--url` is a runtime binary arg; `pake <url>` build command unchanged                          |
| III. Config-Driven via `pake.json`      | ✅ PASS | URL override mutates the in-memory `PakeConfig` struct, not the schema                         |
| IV. JS Injection over Page Modification | ✅ PASS | Not applicable to this feature                                                                 |
| V. Cross-Platform Consistency           | ✅ PASS | cert bypass limited to Linux via `#[cfg(target_os = "linux")]`; `--url` works on all platforms |
| VI. Progressive Build Performance       | ✅ PASS | Zero new Cargo crates; no compile time impact                                                  |
| VII. Quality Gates                      | ✅ PASS | New unit tests required (included in tasks.md); `cargo fmt` must pass                          |

**No violations. No Complexity Tracking needed.**

## Project Structure

### Documentation (this feature)

```text
specs/002-kiosk-cert-url-param/
├── plan.md          ← this file
├── research.md      ← Phase 0 output (inline below; no separate file needed)
├── tasks.md         ← already generated
└── spec.md          ← source of requirements
```

_No `data-model.md`, `contracts/`, or `quickstart.md` required — this feature has no new entities, no external API contracts, and no new setup steps for users._

### Source Code Changes

```text
src-tauri/
└── src/
    ├── util.rs              ← ADD: parse_runtime_url() function (~15 lines)
    ├── lib.rs               ← MODIFY: call parse_runtime_url(), validate, mutate pake_config (~12 lines)
    └── app/
        └── window.rs        ← MODIFY: effective_ignore_cert logic (~3 lines changed)

docs/
├── cli-usage.md             ← ADD: "Runtime Flags" section
└── cli-usage_CN.md          ← ADD: same section in Chinese

tests/unit/
├── runtime-url-arg.test.ts  ← NEW: URL arg parsing unit tests
└── kiosk-cert-bypass.test.ts ← NEW: cert bypass heuristic unit tests
```

**Total lines changed (estimate)**: ~60 lines across 2 Rust source files + 30 lines docs + 50 lines tests.

## Phase 0: Research

_All resolved from existing codebase — no external research agents needed._

### Decision Log

| Decision                 | Choice                                                                  | Rationale                                                                           |
| ------------------------ | ----------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| Args parsing library     | `std::env::args()` (stdlib)                                             | No new crate needed; `--url` is the only runtime flag                               |
| URL validation           | `url::Url::parse()` (already in Tauri transitive deps)                  | Zero cost; already available                                                        |
| Config mutation strategy | Direct field assignment: `pake_config.windows[0].url = s`               | Simplest possible approach; `url` is `String`                                       |
| Cert bypass trigger      | `window_config.fullscreen \|\| window_config.ignore_certificate_errors` | Single `\|\|` — no new fields, no new state                                         |
| New Cargo dependencies   | None                                                                    | `url` crate already present via Tauri                                               |
| Clap / structopt         | Rejected                                                                | YAGNI — adding a full arg parsing framework for one flag would violate Principle VI |

## Phase 1: Design

### US1 — Runtime `--url` Flag

**Approach**: Add a single function `parse_runtime_url()` in `util.rs` that:

1. Iterates `std::env::args()` looking for `"--url"` followed by the next token
2. Returns `None` if flag is absent or value is empty/whitespace (with stderr warning for empty)
3. Returns `Some(String)` otherwise (raw string, not yet validated)

In `lib.rs`, immediately after `get_pake_config()`:

1. Call `parse_runtime_url()`
2. If `Some(raw)`: validate with `url::Url::parse()` — exit(1) on parse failure; exit(1) if scheme ≠ http/https
3. If valid: print `[Pake] URL overridden at runtime: {raw}` to stdout; set `pake_config.windows[0].url = raw`
4. All downstream code picks up the mutation automatically — no further changes

**Why this is simple**: The `pake_config` struct is owned and mutable in `lib.rs`; mutating one field before any Tauri builder code runs is the minimal possible intervention point. No new types, no new traits, no wrappers.

```
lib.rs call flow (new code highlighted with >>>):

get_pake_config()
>>> parse_runtime_url()  →  validate  →  mutate pake_config.windows[0].url
tauri::Builder::default()  (reads pake_config as before)
MultiWindowState::new(pake_config, ...)  (picks up mutated URL)
```

### US2 — Kiosk Cert Auto-Bypass

**Approach**: In `src-tauri/src/app/window.rs`, inside `build_window()`, before the existing cert-bypass block, compute `effective_ignore_cert`:

```rust
// Linux-only: fullscreen (kiosk) mode also triggers cert bypass
#[cfg(target_os = "linux")]
let effective_ignore_cert =
    window_config.ignore_certificate_errors || window_config.fullscreen;

#[cfg(not(target_os = "linux"))]
let effective_ignore_cert = window_config.ignore_certificate_errors;

// Replace `if window_config.ignore_certificate_errors {` with:
if effective_ignore_cert {
    // existing platform branches unchanged
}
```

**Why this is simple**: The existing cert-bypass block already handles all three platforms correctly. We only need to change the condition that guards it. The `fullscreen` field is already in scope (`window_config` is a reference available throughout `build_window`). Net diff: +5 lines, -1 line.

### No data-model.md Required

No new entities, database tables, config schema fields, or public API contracts are introduced. `pake.json` schema is unchanged. The runtime `--url` override is ephemeral (process lifetime only).

### No contracts/ Required

This feature adds no HTTP endpoints, no IPC commands, no public library APIs, and no CLI build-time flags. The runtime `--url` flag is documented in prose in `docs/`.

## Complexity Tracking

_No Constitution violations — section intentionally empty._
