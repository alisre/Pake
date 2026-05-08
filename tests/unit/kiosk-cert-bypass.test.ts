/**
 * Unit tests for the kiosk mode certificate auto-bypass heuristic.
 *
 * On Linux, when a Pake binary is built with `fullscreen: true` in pake.json,
 * the WebKitGTK WebView should automatically receive `--ignore-certificate-errors`
 * even if `ignore_certificate_errors` is not explicitly set to true.
 *
 * This file tests the decision logic (the `effective_ignore_cert` computation)
 * that is implemented in `src-tauri/src/app/window.rs`.
 */

import { describe, it, expect } from 'vitest';

type WindowConfigSubset = {
  fullscreen: boolean;
  ignore_certificate_errors: boolean;
};

/**
 * Mirror of the `effective_ignore_cert` computation in Rust's `build_window()`:
 *
 *   #[cfg(target_os = "linux")]
 *   let effective_ignore_cert =
 *       window_config.ignore_certificate_errors || window_config.fullscreen;
 *
 *   #[cfg(not(target_os = "linux"))]
 *   let effective_ignore_cert = window_config.ignore_certificate_errors;
 */
function computeEffectiveIgnoreCert(
  config: WindowConfigSubset,
  platform: 'linux' | 'macos' | 'windows',
): boolean {
  if (platform === 'linux') {
    // Kiosk auto-bypass: fullscreen implies cert bypass on Linux
    return config.ignore_certificate_errors || config.fullscreen;
  }
  // macOS and Windows: only explicit flag applies
  return config.ignore_certificate_errors;
}

// ---------------------------------------------------------------------------
// Linux: kiosk (fullscreen: true) auto-bypass
// ---------------------------------------------------------------------------

describe('effective_ignore_cert — Linux platform', () => {
  it('enables cert bypass when fullscreen=true and ignore_certificate_errors=false (kiosk auto-bypass)', () => {
    const config: WindowConfigSubset = {
      fullscreen: true,
      ignore_certificate_errors: false,
    };
    expect(computeEffectiveIgnoreCert(config, 'linux')).toBe(true);
  });

  it('does NOT enable cert bypass when fullscreen=false and ignore_certificate_errors=false', () => {
    const config: WindowConfigSubset = {
      fullscreen: false,
      ignore_certificate_errors: false,
    };
    expect(computeEffectiveIgnoreCert(config, 'linux')).toBe(false);
  });

  it('enables cert bypass when ignore_certificate_errors=true regardless of fullscreen (explicit opt-in preserved)', () => {
    const configFullscreen: WindowConfigSubset = {
      fullscreen: true,
      ignore_certificate_errors: true,
    };
    const configWindowed: WindowConfigSubset = {
      fullscreen: false,
      ignore_certificate_errors: true,
    };
    expect(computeEffectiveIgnoreCert(configFullscreen, 'linux')).toBe(true);
    expect(computeEffectiveIgnoreCert(configWindowed, 'linux')).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// macOS: auto-bypass does NOT apply — only explicit flag
// ---------------------------------------------------------------------------

describe('effective_ignore_cert — macOS platform', () => {
  it('does NOT enable cert bypass when fullscreen=true (kiosk mode has no effect on macOS)', () => {
    const config: WindowConfigSubset = {
      fullscreen: true,
      ignore_certificate_errors: false,
    };
    expect(computeEffectiveIgnoreCert(config, 'macos')).toBe(false);
  });

  it('enables cert bypass only when ignore_certificate_errors=true on macOS', () => {
    const config: WindowConfigSubset = {
      fullscreen: true,
      ignore_certificate_errors: true,
    };
    expect(computeEffectiveIgnoreCert(config, 'macos')).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// Windows: auto-bypass does NOT apply — only explicit flag
// ---------------------------------------------------------------------------

describe('effective_ignore_cert — Windows platform', () => {
  it('does NOT enable cert bypass when fullscreen=true (kiosk mode has no effect on Windows)', () => {
    const config: WindowConfigSubset = {
      fullscreen: true,
      ignore_certificate_errors: false,
    };
    expect(computeEffectiveIgnoreCert(config, 'windows')).toBe(false);
  });

  it('enables cert bypass only when ignore_certificate_errors=true on Windows', () => {
    const config: WindowConfigSubset = {
      fullscreen: false,
      ignore_certificate_errors: true,
    };
    expect(computeEffectiveIgnoreCert(config, 'windows')).toBe(true);
  });
});
