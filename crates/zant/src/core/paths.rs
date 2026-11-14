//! Per-user locations for app data, following each platform's convention.

use std::path::PathBuf;

/// The OS conventions encoded by `app_data_dir`. Split out (instead of compiling
/// only the host branch) so tests can exercise every platform on any machine.
/// Only one variant is constructed on a given host (via `PLATFORM`), so the
/// others are dead code outside tests.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Platform {
    Macos,
    Windows,
    Other,
}

#[cfg(target_os = "macos")]
const PLATFORM: Platform = Platform::Macos;
#[cfg(target_os = "windows")]
const PLATFORM: Platform = Platform::Windows;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const PLATFORM: Platform = Platform::Other;

/// Directory for an app's settings and databases (not created):
///
/// - macOS: `~/Library/Application Support/<app_id>`
/// - Windows: `%APPDATA%\<app_id>` (roaming profile)
/// - elsewhere: `$XDG_DATA_HOME/<app_id>` or `~/.local/share/<app_id>`
///
/// `app_id` is usually the bundle id (`com.example.app`). Returns null when
/// the environment has no home / profile directory.
pub fn app_data_dir(app_id: &str) -> Option<PathBuf> {
    app_data_dir_with(PLATFORM, |key| std::env::var(key).ok(), app_id)
}

fn app_data_dir_with(
    platform: Platform,
    environ: impl Fn(&str) -> Option<String>,
    app_id: &str,
) -> Option<PathBuf> {
    match platform {
        Platform::Macos => {
            let home = environ("HOME")?;
            Some(join(platform, &[&home, "Library", "Application Support", app_id]).into())
        }
        Platform::Windows => {
            if let Some(appdata) = environ("APPDATA") {
                return Some(join(platform, &[&appdata, app_id]).into());
            }
            let profile = environ("USERPROFILE")?;
            Some(join(platform, &[&profile, "AppData", "Roaming", app_id]).into())
        }
        Platform::Other => {
            if let Some(data) = environ("XDG_DATA_HOME") {
                return Some(join(platform, &[&data, app_id]).into());
            }
            let home = environ("HOME")?;
            Some(join(platform, &[&home, ".local", "share", app_id]).into())
        }
    }
}

/// Combines components with the native separator, skipping
/// empty components and not duplicating a separator between two components.
fn join(platform: Platform, parts: &[&str]) -> String {
    let sep = match platform {
        Platform::Windows => '\\',
        Platform::Macos | Platform::Other => '/',
    };
    let mut out = String::new();
    let mut prev: Option<&str> = None;
    for part in parts {
        if part.is_empty() {
            continue;
        }
        match prev {
            None => out.push_str(part),
            Some(prev_part) => {
                let prev_sep = is_sep(platform, prev_part.as_bytes()[prev_part.len() - 1]);
                let this_sep = is_sep(platform, part.as_bytes()[0]);
                if !prev_sep && !this_sep {
                    out.push(sep);
                }
                if prev_sep && this_sep {
                    // Skip the redundant separator of this component.
                    out.push_str(&part[1..]);
                } else {
                    out.push_str(part);
                }
            }
        }
        prev = Some(part);
    }
    out
}

fn is_sep(platform: Platform, byte: u8) -> bool {
    match platform {
        // Windows accepts both separators.
        Platform::Windows => byte == b'/' || byte == b'\\',
        Platform::Macos | Platform::Other => byte == b'/',
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_data_dir_uses_the_platform_convention() {
        let env = |key: &str| match key {
            "HOME" => Some("/Users/me".to_string()),
            "APPDATA" => Some("C:\\Users\\me\\AppData\\Roaming".to_string()),
            _ => None,
        };
        let dir = app_data_dir_with(PLATFORM, env, "com.example.app").expect("a home directory");
        if cfg!(target_os = "macos") {
            assert_eq!(
                PathBuf::from("/Users/me/Library/Application Support/com.example.app"),
                dir
            );
        } else if cfg!(target_os = "windows") {
            assert_eq!(
                PathBuf::from("C:\\Users\\me\\AppData\\Roaming\\com.example.app"),
                dir
            );
        }
    }

    /// Every `app_data_dir` branch, checked on any host.
    #[test]
    fn app_data_dir_platform_branches() {
        let full_env = |key: &str| match key {
            "HOME" => Some("/home/me".to_string()),
            "APPDATA" => Some("C:\\Users\\me\\AppData\\Roaming".to_string()),
            "USERPROFILE" => Some("C:\\Users\\me".to_string()),
            "XDG_DATA_HOME" => Some("/home/me/.local/share-custom".to_string()),
            _ => None,
        };
        assert_eq!(
            PathBuf::from("/home/me/Library/Application Support/com.example.app"),
            app_data_dir_with(Platform::Macos, full_env, "com.example.app").unwrap()
        );
        assert_eq!(
            PathBuf::from("C:\\Users\\me\\AppData\\Roaming\\com.example.app"),
            app_data_dir_with(Platform::Windows, full_env, "com.example.app").unwrap()
        );
        assert_eq!(
            PathBuf::from("/home/me/.local/share-custom/com.example.app"),
            app_data_dir_with(Platform::Other, full_env, "com.example.app").unwrap()
        );

        // Windows falls back to USERPROFILE when APPDATA is unset.
        let no_appdata = |key: &str| match key {
            "USERPROFILE" => Some("C:\\Users\\me".to_string()),
            _ => None,
        };
        assert_eq!(
            PathBuf::from("C:\\Users\\me\\AppData\\Roaming\\app"),
            app_data_dir_with(Platform::Windows, no_appdata, "app").unwrap()
        );

        // Other platforms fall back to ~/.local/share when XDG_DATA_HOME is unset.
        let home_only = |key: &str| match key {
            "HOME" => Some("/home/me".to_string()),
            _ => None,
        };
        assert_eq!(
            PathBuf::from("/home/me/.local/share/app"),
            app_data_dir_with(Platform::Other, home_only, "app").unwrap()
        );

        // No home / profile directory: null.
        let empty = |_: &str| None::<String>;
        assert_eq!(None, app_data_dir_with(Platform::Macos, empty, "app"));
        assert_eq!(None, app_data_dir_with(Platform::Windows, empty, "app"));
        assert_eq!(None, app_data_dir_with(Platform::Other, empty, "app"));
    }
}
