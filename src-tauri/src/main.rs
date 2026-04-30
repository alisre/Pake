#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

/// 解析 Pake 运行时 CLI 参数。
///
/// 支持的 flag（仅在已打包二进制启动时生效，不影响 `pake` 构建工具自身）：
///
/// - `--url <URL>`：覆盖 `pake.json` 中编译期 URL，本次进程内有效，URL 必须以
///   `http://` 或 `https://` 开头。
/// - `--ignore-cert`：启用 WebView 的 `--ignore-certificate-errors`，对所有
///   HTTPS 请求（含跳转）跳过证书校验。与 `pake.json.windows[0].ignore_certificate_errors`
///   是 OR 合并关系（任一为真即生效）。
///
/// 抽成纯函数便于在 `#[cfg(test)]` 下做单元测试。
pub(crate) fn parse_runtime_args<I, S>(args: I) -> Result<RuntimeOverrides, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut url: Option<String> = None;
    let mut ignore_cert = false;

    let mut iter = args.into_iter();
    while let Some(raw) = iter.next() {
        let arg = raw.as_ref();
        match arg {
            "--url" => match iter.next() {
                Some(value) => {
                    let v = value.as_ref().to_string();
                    if !(v.starts_with("http://") || v.starts_with("https://")) {
                        return Err(format!(
                            "--url 必须以 http:// 或 https:// 开头，收到: {v}"
                        ));
                    }
                    url = Some(v);
                }
                None => return Err("--url 参数缺少 URL 值".to_string()),
            },
            "--ignore-cert" => ignore_cert = true,
            _ => {} // 未识别参数原样保留，供后续逻辑/Tauri 自身使用
        }
    }

    Ok(RuntimeOverrides { url, ignore_cert })
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct RuntimeOverrides {
    pub url: Option<String>,
    pub ignore_cert: bool,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse_runtime_args(&args) {
        Ok(overrides) => {
            if let Some(url) = overrides.url {
                std::env::set_var("PAKE_RUNTIME_URL", url);
            }
            if overrides.ignore_cert {
                std::env::set_var("PAKE_IGNORE_CERT", "1");
            }
        }
        Err(msg) => {
            eprintln!("[browser] {msg}");
            std::process::exit(1);
        }
    }

    app_lib::run()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<RuntimeOverrides, String> {
        parse_runtime_args(args.iter().copied())
    }

    #[test]
    fn empty_args_yields_no_overrides() {
        assert_eq!(
            parse(&[]).unwrap(),
            RuntimeOverrides {
                url: None,
                ignore_cert: false
            }
        );
    }

    #[test]
    fn url_https_is_accepted() {
        let r = parse(&["--url", "https://example.com"]).unwrap();
        assert_eq!(r.url.as_deref(), Some("https://example.com"));
        assert!(!r.ignore_cert);
    }

    #[test]
    fn url_http_is_accepted() {
        let r = parse(&["--url", "http://intra.local:8080/path?q=1"]).unwrap();
        assert_eq!(r.url.as_deref(), Some("http://intra.local:8080/path?q=1"));
    }

    #[test]
    fn url_missing_value_returns_error() {
        let err = parse(&["--url"]).unwrap_err();
        assert!(err.contains("缺少 URL 值"));
    }

    #[test]
    fn url_non_http_protocol_returns_error() {
        let err = parse(&["--url", "ftp://x"]).unwrap_err();
        assert!(err.contains("http://") && err.contains("https://"));
    }

    #[test]
    fn ignore_cert_flag_only() {
        let r = parse(&["--ignore-cert"]).unwrap();
        assert!(r.ignore_cert);
        assert!(r.url.is_none());
    }

    #[test]
    fn url_then_ignore_cert() {
        let r = parse(&["--url", "https://a", "--ignore-cert"]).unwrap();
        assert_eq!(r.url.as_deref(), Some("https://a"));
        assert!(r.ignore_cert);
    }

    #[test]
    fn ignore_cert_then_url() {
        let r = parse(&["--ignore-cert", "--url", "https://b"]).unwrap();
        assert_eq!(r.url.as_deref(), Some("https://b"));
        assert!(r.ignore_cert);
    }

    #[test]
    fn unknown_args_are_ignored() {
        let r = parse(&["--foo", "bar", "--url", "https://c", "--baz"]).unwrap();
        assert_eq!(r.url.as_deref(), Some("https://c"));
    }
}
