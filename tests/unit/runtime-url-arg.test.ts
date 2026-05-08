/**
 * Unit tests for runtime --url argument parsing logic.
 *
 * These tests validate the expected behaviour of the Rust `parse_runtime_url()`
 * function by specifying the contract in TypeScript (mirrors the Rust logic).
 *
 * Because the Rust function reads `std::env::args()` (process-level), we test
 * the equivalent parsing logic here as a contract spec.  The actual Rust
 * function is kept simple enough that this spec fully documents it.
 */

import { describe, it, expect } from 'vitest';

/**
 * Pure TypeScript mirror of the Rust `parse_runtime_url()` logic for testing.
 * Returns the URL string if --url flag is present with a non-empty value,
 * or null otherwise.
 */
function parseRuntimeUrl(args: string[]): {
  url: string | null;
  warning?: string;
} {
  // Skip argv[0] (binary name)
  const argv = args.slice(1);

  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--url') {
      const value = argv[i + 1];
      if (!value || value.trim() === '') {
        return {
          url: null,
          warning: '[Pake] Warning: --url value is empty, using baked-in URL',
        };
      }
      return { url: value };
    }
  }

  return { url: null };
}

/**
 * URL validation logic — mirrors Rust validation in lib.rs.
 */
function validateUrl(raw: string): { valid: boolean; error?: string } {
  let parsed: URL;
  try {
    parsed = new URL(raw);
  } catch {
    return {
      valid: false,
      error: `[Pake] Error: --url '${raw}' is not a valid URL`,
    };
  }

  const scheme = parsed.protocol.replace(':', ''); // URL.protocol includes trailing ':'
  if (scheme !== 'http' && scheme !== 'https') {
    return {
      valid: false,
      error:
        '[Pake] Error: --url only accepts http:// or https:// schemes; url_type is immutable after build',
    };
  }

  return { valid: true };
}

// ---------------------------------------------------------------------------
// parse_runtime_url() contract tests
// ---------------------------------------------------------------------------

describe('parseRuntimeUrl — --url flag parsing', () => {
  it('returns the URL when --url is provided with a valid value', () => {
    const result = parseRuntimeUrl(['./myapp', '--url', 'https://valid.example.com']);
    expect(result.url).toBe('https://valid.example.com');
    expect(result.warning).toBeUndefined();
  });

  it('returns null when no --url flag is present', () => {
    const result = parseRuntimeUrl(['./myapp']);
    expect(result.url).toBeNull();
    expect(result.warning).toBeUndefined();
  });

  it('returns null and warning when --url is followed by empty string', () => {
    const result = parseRuntimeUrl(['./myapp', '--url', '']);
    expect(result.url).toBeNull();
    expect(result.warning).toBe(
      '[Pake] Warning: --url value is empty, using baked-in URL',
    );
  });

  it('returns null and warning when --url is followed by whitespace-only string', () => {
    const result = parseRuntimeUrl(['./myapp', '--url', '   ']);
    expect(result.url).toBeNull();
    expect(result.warning).toBe(
      '[Pake] Warning: --url value is empty, using baked-in URL',
    );
  });

  it('returns null and warning when --url is the last argument (no value follows)', () => {
    const result = parseRuntimeUrl(['./myapp', '--url']);
    expect(result.url).toBeNull();
    expect(result.warning).toBe(
      '[Pake] Warning: --url value is empty, using baked-in URL',
    );
  });

  it('correctly parses --url when other flags precede it', () => {
    const result = parseRuntimeUrl(['./myapp', '--some-flag', 'val', '--url', 'https://foo.com']);
    expect(result.url).toBe('https://foo.com');
  });
});

// ---------------------------------------------------------------------------
// URL validation contract tests
// ---------------------------------------------------------------------------

describe('validateUrl — URL scheme and format validation', () => {
  it('accepts https:// URLs', () => {
    const result = validateUrl('https://192.168.1.100/dashboard');
    expect(result.valid).toBe(true);
    expect(result.error).toBeUndefined();
  });

  it('accepts http:// URLs', () => {
    const result = validateUrl('http://internal.example.com');
    expect(result.valid).toBe(true);
    expect(result.error).toBeUndefined();
  });

  it('rejects syntactically invalid URLs with a descriptive error', () => {
    const result = validateUrl('not-a-url');
    expect(result.valid).toBe(false);
    expect(result.error).toContain('is not a valid URL');
  });

  it('rejects ftp:// scheme with a scheme-specific error', () => {
    const result = validateUrl('ftp://bad-scheme.com');
    expect(result.valid).toBe(false);
    expect(result.error).toContain('only accepts http:// or https:// schemes');
  });

  it('rejects file:// scheme with a scheme-specific error', () => {
    const result = validateUrl('file:///etc/passwd');
    expect(result.valid).toBe(false);
    expect(result.error).toContain('only accepts http:// or https:// schemes');
  });

  it('accepts URLs with non-standard ports', () => {
    const result = validateUrl('https://192.168.1.1:8443/app');
    expect(result.valid).toBe(true);
  });

  it('accepts URLs with path and query string', () => {
    const result = validateUrl('https://example.com/path?key=value&other=123');
    expect(result.valid).toBe(true);
  });
});
