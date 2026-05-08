<!--
SYNC IMPACT REPORT
==================
Version change: (template / unversioned) → 1.0.0
Modified principles: N/A — initial ratification (all sections new)
Added sections: Core Principles (I–VII), Technology Stack Constraints,
                Development Workflow & Quality Gates, Governance
Removed sections: None
Templates requiring updates:
  ✅ .specify/templates/plan-template.md  — Constitution Check gate aligns with principles I–VII
  ✅ .specify/templates/spec-template.md  — no structural changes required; acceptance scenarios
                                            should reference platform-specific constraints
  ✅ .specify/templates/tasks-template.md — task categories cover CLI, Rust/Tauri, tests, formatting
Follow-up TODOs: None — all placeholders resolved
-->

# Pake Constitution

## Core Principles

### I. Lightweight-First — Electron is Forbidden

Pake MUST use Rust/Tauri v2 as its desktop runtime. Electron, NW.js, or any
other Chromium-bundling framework MUST NOT be introduced under any circumstances.
Every architectural decision MUST prioritize binary size and memory footprint.
The native system WebView (WKWebView on macOS, WebView2 on Windows,
WebKitGTK on Linux) is the only permitted rendering engine.

**Rationale**: The defining value proposition of Pake over Electron is a
dramatically smaller app bundle (typically <5 MB vs 100+ MB) and lower RAM
usage. Violating this principle destroys the project's core identity.

### II. Single-Command Experience

`pake <url>` MUST remain the only required argument. All other options MUST
have sensible defaults so a first-time user never needs to read the docs to
produce a working desktop app. New CLI flags MUST be optional with defaults
derived from the URL or project heuristics. Breaking the one-command contract
requires a major version bump and explicit migration guidance.

**Rationale**: Ease of use is the primary driver of adoption. Complexity
creep in the CLI erodes the project's accessibility to non-Rust developers.

### III. Platform-Native Rendering via Configuration

All window behavior (title bar, fullscreen, resizability, always-on-top,
dark mode, zoom, incognito, proxy, internal/external URL routing) MUST be
expressed through `pake.json` and the platform-specific `tauri.*.conf.json`
files. Rust runtime code MUST read these configuration files at startup via
`util.rs` rather than hard-coding values. The `PlatformSpecific<T>` generic
MUST be used wherever macOS/Windows/Linux behavior diverges.

**Rationale**: Configuration-driven design keeps the Rust codebase stable
while allowing per-app customization without recompilation of shared logic.

### IV. JS Injection over Page Modification

All browser-side enhancements (link interception, OAuth flows, theme
switching, toast notifications, custom styles, fullscreen toggling) MUST be
implemented as injected scripts under `src-tauri/src/inject/`. The injected
scripts MUST NOT assume or require any cooperation from the target webpage.
Modifying a target website's source, adding server-side proxies, or
requiring API keys from third-party services is prohibited.

**Rationale**: Target webpages are third-party assets. Injection keeps Pake
non-invasive and ensures compatibility with any URL without negotiation.

### V. Cross-Platform Consistency (NON-NEGOTIABLE)

Every feature shipped MUST work on macOS, Windows, and Linux. Platform-
specific code paths are permitted only when the underlying OS API differs;
they MUST be isolated behind `#[cfg(target_os = ...)]` guards in Rust or
`BuilderProvider` / platform-specific `Builder` classes in TypeScript. A
feature that only works on one platform MUST NOT be merged until the other
platforms are handled or the limitation is explicitly documented as a known
constraint.

**Rationale**: Pake's cross-platform promise is a hard requirement for its
CI/CD pipeline and community trust. Partial support causes support burden
and user confusion.

### VI. Progressive Build Performance

The two-phase build strategy (`prepare()` + `build(url)`) MUST be preserved.
`prepare()` installs Rust toolchain dependencies once; `build()` performs
incremental Tauri compilation. New build steps MUST be placed in the correct
phase. Rust dependencies that dramatically increase cold-build time (>2 min
additional on a modern laptop) require explicit justification and a team vote
before merging. CN mirror acceleration (`CARGO_MIRROR`, `npm` registry) MUST
remain opt-in via environment variables, never forced.

**Rationale**: First-time build time is the biggest UX obstacle for new
contributors. Protecting incremental build speed keeps the developer loop
fast.

### VII. Quality Gates (NON-NEGOTIABLE)

Every pull request MUST pass all of the following before merge:

- **TypeScript unit tests**: `pnpm test` (Vitest) — zero failures
- **Rust tests**: `cargo test --manifest-path src-tauri/Cargo.toml` — zero failures
- **TypeScript formatting**: `pnpm prettier --check .` — zero violations
- **Rust formatting**: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` — zero violations
- **Integration test**: `pnpm test:integration` — workflow paths validate correctly

New code MUST include corresponding unit tests. Changes to `BaseBuilder`,
`tauriConfig`, or Rust `app/` modules MUST include or update tests in
`tests/unit/` or `tests/integration/`.

**Rationale**: Automated quality gates protect the reliability of a tool
used to package production apps. Manual review alone is insufficient at
community-contribution scale.

## Technology Stack Constraints

**Permitted technologies** (changes require a constitution amendment):

| Layer                  | Technology     | Version                            |
| ---------------------- | -------------- | ---------------------------------- |
| CLI runtime            | Node.js + pnpm | pnpm ≥ 8                           |
| CLI language           | TypeScript     | 5.x                                |
| Desktop runtime        | Tauri          | v2.x                               |
| Systems language       | Rust           | stable (see `rust-toolchain.toml`) |
| JS bundler (CLI)       | Rollup         | as in `rollup.config.js`           |
| Test framework (TS)    | Vitest         | as in `vitest.config.ts`           |
| Package manager (JS)   | pnpm           | no npm/yarn                        |
| Package manager (Rust) | Cargo          | standard toolchain                 |

**Strictly forbidden**:

- Electron, NW.js, or any bundled-Chromium framework
- `npm` or `yarn` as the JS package manager (pnpm only)
- Direct DOM manipulation of target web pages from Rust
- Storing user credentials or OAuth tokens in plaintext on disk
- Publishing builds that embed secrets or API keys

**Icon pipeline**: `icns2png.py` (Python) is the only permitted icon
conversion tool outside of the `bin/utils/ico.ts` TypeScript pipeline.
Additional icon-handling dependencies require explicit approval.

## Development Workflow & Quality Gates

### Environment Setup

```bash
# Install JS dependencies
pnpm install

# Verify Rust toolchain (auto-installs via rust-toolchain.toml)
rustup show
```

### Build

```bash
# CLI build (TypeScript → dist/)
pnpm build

# Full app build for current platform (after pnpm build)
node dist/cli.js <url> --name MyApp
```

### Testing

```bash
# TypeScript unit tests (watch mode)
pnpm test

# TypeScript unit tests (CI / single run)
pnpm test --run

# Integration tests
pnpm test:integration

# Rust tests
cargo test --manifest-path src-tauri/Cargo.toml
```

### Formatting

```bash
# Check TypeScript/JSON formatting
pnpm prettier --check .

# Apply TypeScript/JSON formatting
pnpm prettier --write .

# Check Rust formatting
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check

# Apply Rust formatting
cargo fmt --manifest-path src-tauri/Cargo.toml
```

### Pull Request Checklist

Before opening a PR, all of the following MUST pass locally:

1. `pnpm build` — compiles without errors
2. `pnpm test --run` — all Vitest tests pass
3. `pnpm prettier --check .` — no formatting issues
4. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` — no formatting issues
5. `cargo test --manifest-path src-tauri/Cargo.toml` — all Rust tests pass
6. Manual smoke test: `node dist/cli.js https://example.com --name Test` produces a runnable app on the target platform

### GitHub Actions CI

Release pipelines run in `.github/workflows/`. They MUST NOT be bypassed
(`--no-verify` or skipping status checks) for any merge into `main`.
The `action.yml` Action is the canonical entry point for GitHub Actions
usage by external consumers; its interface is part of the public API.

## Governance

This Constitution supersedes all other project conventions, README guidance,
and verbal agreements. When a conflict exists between any document and this
Constitution, this Constitution prevails.

**Amendment procedure**:

1. Open a GitHub issue describing the proposed change and rationale.
2. Achieve consensus among active maintainers (minimum 2 approvals).
3. Update this file with the new content and increment the version per
   semantic versioning rules defined below.
4. Update all dependent templates (plan, spec, tasks) in the same PR.
5. Tag the commit `constitution-vX.Y.Z`.

**Versioning policy**:

- **MAJOR**: Principle removed, renamed, or fundamentally redefined;
  breaking change to the CLI public interface.
- **MINOR**: New principle or section added; material expansion of existing
  guidance; new technology added to the permitted stack.
- **PATCH**: Clarification, wording improvement, typo fix, non-semantic
  refinement; template alignment with no governance change.

**Compliance review**: All PRs MUST include a "Constitution Check" section
in the plan (generated by `/speckit.plan`) verifying that the change does
not violate Principles I–VII. Reviewers are responsible for catching
violations during code review.

**License**: MIT. All contributions to this repository are made under the
MIT License. The Constitution itself is part of the repository and therefore
MIT-licensed.

**Version**: 1.0.0 | **Ratified**: 2026-05-08 | **Last Amended**: 2026-05-08
