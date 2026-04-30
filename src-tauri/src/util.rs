use crate::app::config::PakeConfig;
use std::env;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Config, Manager, WebviewWindow};

/// 应用运行期覆盖到 [`PakeConfig`]。
///
/// 该函数是运行期 CLI 参数与编译期 `pake.json` 之间的**唯一汇点**，负责：
///
/// 1. `PAKE_RUNTIME_URL` 设置时覆盖 `windows[0].url` 并强制 `url_type="web"`。
/// 2. `PAKE_IGNORE_CERT` 设置（任意非空值）时对 `windows[0].ignore_certificate_errors`
///    执行 **OR 合并**，不覆盖已为 `true` 的构建期值。
///
/// **不**进行任何隐式启用（如“localhost 自动忽略证书”）— 安全语义 MUST 由
/// 用户显式选择。
///
/// 抽成独立函数以便在 `#[cfg(test)]` 下做单元测试。
fn apply_runtime_overrides(cfg: &mut PakeConfig) {
    if let Ok(runtime_url) = env::var("PAKE_RUNTIME_URL") {
        if let Some(win) = cfg.windows.first_mut() {
            win.url = runtime_url;
            win.url_type = "web".to_string();
        }
    }

    if env::var("PAKE_IGNORE_CERT")
        .map(|v| !v.is_empty())
        .unwrap_or(false)
    {
        if let Some(win) = cfg.windows.first_mut() {
            win.ignore_certificate_errors |= true;
        }
    }
}

pub fn get_pake_config() -> (PakeConfig, Config) {
    #[cfg(feature = "cli-build")]
    let mut pake_config: PakeConfig = serde_json::from_str(include_str!("../.pake/pake.json"))
        .expect("Failed to parse pake config");

    #[cfg(not(feature = "cli-build"))]
    let mut pake_config: PakeConfig =
        serde_json::from_str(include_str!("../pake.json")).expect("Failed to parse pake config");

    #[cfg(feature = "cli-build")]
    let tauri_config: Config = serde_json::from_str(include_str!("../.pake/tauri.conf.json"))
        .expect("Failed to parse tauri config");

    #[cfg(not(feature = "cli-build"))]
    let tauri_config: Config = serde_json::from_str(include_str!("../tauri.conf.json"))
        .expect("Failed to parse tauri config");

    apply_runtime_overrides(&mut pake_config);

    (pake_config, tauri_config)
}

pub fn get_data_dir(app: &AppHandle, package_name: String) -> PathBuf {
    {
        let data_dir = app
            .path()
            .config_dir()
            .expect("Failed to get data dirname")
            .join(package_name);

        if !data_dir.exists() {
            std::fs::create_dir(&data_dir)
                .unwrap_or_else(|_| panic!("Can't create dir {}", data_dir.display()));
        }
        data_dir
    }
}

pub fn show_toast(window: &WebviewWindow, message: &str) {
    let script = format!(r#"pakeToast("{message}");"#);
    window.eval(&script).unwrap();
}

pub enum MessageType {
    Start,
    Success,
    Failure,
}

pub fn get_download_message_with_lang(
    message_type: MessageType,
    language: Option<String>,
) -> String {
    let default_start_message = "Start downloading~";
    let chinese_start_message = "开始下载中~";

    let default_success_message = "Download successful, saved to download directory~";
    let chinese_success_message = "下载成功，已保存到下载目录~";

    let default_failure_message = "Download failed, please check your network connection~";
    let chinese_failure_message = "下载失败，请检查你的网络连接~";

    let is_chinese = language
        .as_ref()
        .map(|lang| {
            lang.starts_with("zh")
                || lang.contains("CN")
                || lang.contains("TW")
                || lang.contains("HK")
        })
        .unwrap_or_else(|| {
            // Try multiple environment variables for better system detection
            ["LANG", "LC_ALL", "LC_MESSAGES", "LANGUAGE"]
                .iter()
                .find_map(|var| env::var(var).ok())
                .map(|lang| {
                    lang.starts_with("zh")
                        || lang.contains("CN")
                        || lang.contains("TW")
                        || lang.contains("HK")
                })
                .unwrap_or(false)
        });

    if is_chinese {
        match message_type {
            MessageType::Start => chinese_start_message,
            MessageType::Success => chinese_success_message,
            MessageType::Failure => chinese_failure_message,
        }
    } else {
        match message_type {
            MessageType::Start => default_start_message,
            MessageType::Success => default_success_message,
            MessageType::Failure => default_failure_message,
        }
    }
    .to_string()
}

// Check if the file exists, if it exists, add a number to file name
pub fn check_file_or_append(file_path: &str) -> String {
    let mut new_path = PathBuf::from(file_path);
    let mut counter = 0;

    while new_path.exists() {
        let file_stem = new_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let extension = new_path
            .extension()
            .map(|e| e.to_string_lossy().to_string());
        let parent_dir = new_path.parent().unwrap_or(Path::new(""));

        let new_file_stem = match file_stem.rfind('-') {
            Some(index) if file_stem[index + 1..].parse::<u32>().is_ok() => {
                let base_name = &file_stem[..index];
                counter = file_stem[index + 1..].parse::<u32>().unwrap() + 1;
                format!("{base_name}-{counter}")
            }
            _ => {
                counter += 1;
                format!("{file_stem}-{counter}")
            }
        };

        new_path = match &extension {
            Some(ext) => parent_dir.join(format!("{new_file_stem}.{ext}")),
            None => parent_dir.join(new_file_stem),
        };
    }

    new_path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::apply_runtime_overrides;
    use crate::app::config::PakeConfig;
    use std::env;
    use std::sync::Mutex;

    // env 是进程级共享，多个测试串行避免相互污染
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn make_cfg(initial_ignore: bool) -> PakeConfig {
        let mut cfg: PakeConfig = serde_json::from_str(include_str!("../pake.json"))
            .expect("parse pake.json fixture");
        if let Some(w) = cfg.windows.first_mut() {
            w.url = "https://prod.example.com".into();
            w.url_type = "web".into();
            w.ignore_certificate_errors = initial_ignore;
        }
        cfg
    }

    fn clear_env() {
        env::remove_var("PAKE_RUNTIME_URL");
        env::remove_var("PAKE_IGNORE_CERT");
    }

    #[test]
    fn no_env_keeps_config_unchanged() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_env();
        let mut cfg = make_cfg(false);
        apply_runtime_overrides(&mut cfg);
        let w = cfg.windows.first().unwrap();
        assert_eq!(w.url, "https://prod.example.com");
        assert_eq!(w.url_type, "web");
        assert!(!w.ignore_certificate_errors);
    }

    #[test]
    fn runtime_url_overrides_and_forces_web_type() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_env();
        env::set_var("PAKE_RUNTIME_URL", "https://stage.intra.example.com");
        let mut cfg = make_cfg(false);
        apply_runtime_overrides(&mut cfg);
        let w = cfg.windows.first().unwrap();
        assert_eq!(w.url, "https://stage.intra.example.com");
        assert_eq!(w.url_type, "web");
        clear_env();
    }

    #[test]
    fn ignore_cert_env_enables_flag() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_env();
        env::set_var("PAKE_IGNORE_CERT", "1");
        let mut cfg = make_cfg(false);
        apply_runtime_overrides(&mut cfg);
        assert!(cfg.windows.first().unwrap().ignore_certificate_errors);
        clear_env();
    }

    #[test]
    fn ignore_cert_or_merge_does_not_clear_compiletime_true() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_env();
        let mut cfg = make_cfg(true);
        apply_runtime_overrides(&mut cfg);
        assert!(cfg.windows.first().unwrap().ignore_certificate_errors);
    }

    #[test]
    fn empty_ignore_cert_env_does_not_enable() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_env();
        env::set_var("PAKE_IGNORE_CERT", "");
        let mut cfg = make_cfg(false);
        apply_runtime_overrides(&mut cfg);
        assert!(!cfg.windows.first().unwrap().ignore_certificate_errors);
        clear_env();
    }
}
