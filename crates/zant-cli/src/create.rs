//! Port of the `zant create` half of `tools/zant/main.zig`: project name /
//! bundle id validation, interactive prompts, the scaffold template and the
//! initial `app.config.json`.

use crate::app_config::bundle_error;
use crate::project::Language;
use crate::scaffold;
use crate::{
    COLOR_RED, COLOR_RESET, escape_json, fail, ok, path_exists, prompt, read_line, sep, step,
    write_file, zant_root,
};
use std::io::Write as _;
use std::path::{Path, PathBuf};

pub struct CreateOptions {
    pub name: String,
    pub yes: bool,
    pub force: bool,
    pub lang: Option<String>,
    pub title: Option<String>,
    pub style: Option<String>,
    pub bundle_id: Option<String>,
    pub sandbox: Option<String>,
    pub sign: Option<String>,
}

pub fn cmd_create(options: CreateOptions) -> std::io::Result<()> {
    let language_arg = options
        .lang
        .as_deref()
        .map(|value| Language::parse(value).unwrap_or_else(|| fail("语言必须是 zig 或 ts")));
    let name = options.name.as_str();
    if !valid_name(name) {
        fail("项目名只能以字母开头，并包含字母、数字、-、_");
    }

    let cwd = std::env::current_dir()?;
    let target = cwd.join(name);
    if path_exists(&target) && !options.force {
        fail(format!(
            "目录已存在: {}（使用 --force 覆盖）",
            target.to_string_lossy()
        ));
    }

    let default_title = title_from_name(name);
    let slug = slug_from_name(name);
    let zig_name = zig_name_from_name(name);
    let default_bundle = format!("dev.zant.{slug}");

    let mut language = language_arg.unwrap_or(Language::Zig);
    let mut title = options.title.clone().unwrap_or(default_title);
    let mut style = options
        .style
        .clone()
        .unwrap_or_else(|| "native".to_string());
    // app identifier (bundle_id): the macOS bundle id and the Windows data
    // folder name. macOS-only settings (sandbox, signing) are added by
    // `zant run` / `zant package` on a Mac, see generate::complete_platform_config.
    let mut bundle = options.bundle_id.clone().unwrap_or(default_bundle);

    if !options.yes {
        eprintln!("\nZant UI 项目生成器\n");
        if language_arg.is_none() {
            eprintln!("语言：\n  1) Zig\n  2) TypeScript + TSX");
            loop {
                let value = prompt("选择", "1");
                let choice = match value.as_str() {
                    "1" => "zig",
                    "2" => "ts",
                    other => other,
                };
                if let Some(chosen) = Language::parse(choice) {
                    language = chosen;
                    break;
                }
                eprintln!("请输入 1（Zig）或 2（TypeScript + TSX）");
            }
        }
        if options.title.is_none() {
            title = prompt("应用标题", &title);
        }
        if options.style.is_none() {
            eprintln!(
                "\n窗口风格:\n  1) 原生标题栏\n  2) 隐藏标题栏（全尺寸内容 + 系统窗口按钮：macOS 左上角红黄绿，Windows 右上角最小化 / 最大化 / 关闭）\n  3) 无边框（movable_by_background 时可拖动背景移动；macOS Cmd+Q、Windows Alt+F4 退出）"
            );
            loop {
                let value = prompt("选择", "1");
                match value.as_str() {
                    "1" | "native" => {
                        style = "native".to_string();
                        break;
                    }
                    "2" | "hidden-titlebar" => {
                        style = "hidden-titlebar".to_string();
                        break;
                    }
                    "3" | "borderless" => {
                        style = "borderless".to_string();
                        break;
                    }
                    _ => {}
                }
            }
        }
        if options.bundle_id.is_none() {
            bundle = ask_bundle(&bundle);
        }
    }
    if style == "2" {
        style = "hidden-titlebar".to_string();
    }
    if style == "3" {
        style = "borderless".to_string();
    }
    if !["native", "hidden-titlebar", "borderless"].contains(&style.as_str()) {
        fail("窗口风格必须是 native、hidden-titlebar 或 borderless");
    }
    if title.is_empty() {
        fail("应用标题不能为空");
    }
    if let Some(why) = bundle_error(&bundle) {
        fail(format!("应用标识 {bundle} 不符合规范：{why}"));
    }

    // everything is validated: only now replace an existing folder (--force)
    if path_exists(&target) {
        let _ = std::fs::remove_dir_all(&target);
    }
    step(&format!("创建 {}", target.to_string_lossy()));
    std::fs::create_dir_all(&target)?;
    ok("目录已创建");
    std::fs::create_dir_all(target.join("src/components"))?;

    step("写入模板、配置和自定义组件");
    let zroot = zant_root();
    let mut zant_rel = zant_dependency_path(&target, zroot)?;
    zant_rel = zant_rel.replace('\\', "/");
    scaffold::write(&scaffold::Options {
        framework: zroot.to_string_lossy().into_owned(),
        target: target.to_string_lossy().into_owned(),
        language,
        title: title.clone(),
        bundle: bundle.clone(),
        zig_name,
        dependency: zant_rel,
        hidden_titlebar: style == "hidden-titlebar",
    })?;

    // optional macOS section when --sandbox / --sign were given
    let sandbox_arg = options
        .sandbox
        .as_deref()
        .map(|value| value == "true" || value == "yes");
    let macos_section = if sandbox_arg.is_some() || options.sign.is_some() {
        format!(
            ",\n  \"macos\": {{\n    \"sandbox\": {}{}\n  }}",
            sandbox_arg.unwrap_or(false),
            match &options.sign {
                Some(id) => format!(",\n    \"signing_identity\": \"{}\"", escape_json(id)),
                None => String::new(),
            }
        )
    } else {
        String::new()
    };
    let config_json = format!(
        "{{\n  \"title\": \"{}\",\n  \"bundle_id\": \"{}\",\n  \"version\": \"0.1.0\",\n  \"window\": {{\n    \"style\": \"{}\",\n    \"width\": 680,\n    \"height\": 700,\n    \"min_width\": 600,\n    \"min_height\": 620,\n    \"background\": \"#f5f5f5\",\n    \"resizable\": true,\n    \"always_on_top\": false,\n    \"movable_by_background\": false,\n    \"center\": true\n  }}{}\n}}\n",
        escape_json(&title),
        bundle,
        style,
        macos_section
    );
    write_file(&target.join("app.config.json"), config_json.as_bytes())?;

    // platform files (Info.plist, entitlements) are generated by `zant run`
    let cfg = crate::load_config(&target.to_string_lossy())?;
    crate::generate::write_config_source(&target.to_string_lossy(), &cfg)?;
    ok("app.config.json 已生成");
    ok(&format!("项目已生成: {}", target.to_string_lossy()));
    crate::dim_line(
        "macOS 专属设置（沙盒、签名）在 Mac 上首次 zant run / zant package 时补充到 app.config.json",
    );
    if crate::release_home().is_some() {
        eprint!("\n下一步：\n  cd {name}\n  zant run\n\n");
    } else {
        let zroot = zroot.to_string_lossy();
        eprint!(
            "\n下一步：\n  cd {name}\n  {zroot}{sep}target{sep}debug{sep}zant run\n\n",
            sep = sep()
        );
    }
    Ok(())
}

/// Asks for the app identifier until it follows the rules; three invalid
/// answers end `zant create` before anything is written. The suggested
/// default (from the project name) is offered only when it is valid itself.
fn ask_bundle(suggested: &str) -> String {
    let default = if bundle_error(suggested).is_none() {
        suggested.to_string()
    } else {
        String::new()
    };
    for _ in 0..3 {
        eprintln!();
        let value = if !default.is_empty() {
            prompt("应用标识（反向域名，如 com.xx.test）", &default)
        } else {
            eprint!("应用标识（反向域名，如 com.xx.test）: ");
            let _ = std::io::stderr().flush();
            read_line().trim_matches(' ').to_string()
        };
        if value.is_empty() {
            eprintln!("  {COLOR_RED}✗{COLOR_RESET} 应用标识不能为空，请输入（如 com.xx.test）");
            continue;
        }
        if let Some(why) = bundle_error(&value) {
            eprintln!("  {COLOR_RED}✗{COLOR_RESET} “{value}”不符合规范：{why}，请重新输入");
            continue;
        }
        return value;
    }
    fail("应用标识连续 3 次不符合规范，未创建项目")
}

/// Path of Zant UI for the generated Cargo.toml, which only accepts paths
/// relative to the project. On Windows a project on another drive than Zant UI
/// has no relative path: link Zant UI into the project with a directory
/// junction (no admin rights needed) and refer to that.
fn zant_dependency_path(target: &Path, zroot: &Path) -> std::io::Result<String> {
    let rel = relative_path(target, zroot);
    if !rel.is_absolute() {
        return Ok(rel.to_string_lossy().into_owned());
    }
    let dir = target.join(".zant");
    std::fs::create_dir_all(&dir)?;
    let link = dir.join("zant-ui");
    let capture = crate::run_capture(&[
        "cmd".to_string(),
        "/c".to_string(),
        "mklink".to_string(),
        "/J".to_string(),
        link.to_string_lossy().into_owned(),
        zroot.to_string_lossy().into_owned(),
    ])
    .unwrap_or_else(|err| {
        fail(format!(
            "项目和 Zant UI 不在同一个盘，创建目录联接失败（{err}）；请把项目建在 {} 所在的盘",
            zroot.to_string_lossy()
        ))
    });
    if !capture.success {
        fail(format!(
            "项目和 Zant UI 不在同一个盘，创建目录联接失败：{}",
            capture.stderr
        ));
    }
    ok("项目和 Zant UI 不在同一个盘：已用目录联接 .zant/zant-ui 引用 Zant UI");
    Ok(".zant/zant-ui".to_string())
}

/// Relative path from `from` to `to` (a `..`-walking path); absolute when the
/// two have no common root (different drives), like `std.fs.path.relative`.
fn relative_path(from: &Path, to: &Path) -> PathBuf {
    let from = std::fs::canonicalize(from).unwrap_or_else(|_| from.to_path_buf());
    let to = std::fs::canonicalize(to).unwrap_or_else(|_| to.to_path_buf());
    let from_components: Vec<_> = from.components().collect();
    let to_components: Vec<_> = to.components().collect();
    let mut common = 0;
    while common < from_components.len()
        && common < to_components.len()
        && from_components[common] == to_components[common]
    {
        common += 1;
    }
    if common == 0 {
        return to;
    }
    let mut out = PathBuf::new();
    for _ in common..from_components.len() {
        out.push("..");
    }
    for component in &to_components[common..] {
        out.push(component.as_os_str());
    }
    if out.as_os_str().is_empty() {
        out.push(".");
    }
    out
}

pub fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

pub fn title_from_name(name: &str) -> String {
    let mut out: Vec<char> = name
        .chars()
        .map(|ch| if ch == '-' || ch == '_' { ' ' } else { ch })
        .collect();
    let mut upper = true;
    for ch in out.iter_mut() {
        if *ch == ' ' {
            upper = true;
            continue;
        }
        if upper {
            *ch = ch.to_ascii_uppercase();
            upper = false;
        }
    }
    out.into_iter().collect()
}

pub fn slug_from_name(name: &str) -> String {
    name.chars()
        .map(|ch| match ch {
            'A'..='Z' => ch.to_ascii_lowercase(),
            '_' => '-',
            other => other,
        })
        .collect()
}

pub fn zig_name_from_name(name: &str) -> String {
    name.chars()
        .map(|ch| match ch {
            'A'..='Z' => ch.to_ascii_lowercase(),
            '-' => '_',
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_name_validation_matches_the_zig_rule() {
        for name in ["app", "MyApp", "my-app", "my_app2", "a1"] {
            assert!(valid_name(name), "{name} should be valid");
        }
        for name in [
            "", "1app", "-app", "_app", "my app", "my.app", "app!", "应用",
        ] {
            assert!(!valid_name(name), "{name} should be rejected");
        }
    }

    #[test]
    fn names_become_title_slug_and_snake_name() {
        assert_eq!(title_from_name("my-app"), "My App");
        assert_eq!(title_from_name("my_app"), "My App");
        assert_eq!(title_from_name("demo"), "Demo");
        assert_eq!(slug_from_name("My_App"), "my-app");
        assert_eq!(zig_name_from_name("My-App"), "my_app");
    }

    #[test]
    fn relative_path_walks_up_to_the_framework() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("repo root");
        let target = root.join("target/zant-cli-test/rel/my-app");
        std::fs::create_dir_all(&target).expect("target dir");
        let rel = relative_path(&target, &root);
        assert_eq!(rel, std::path::Path::new("../../../.."));
        assert_eq!(
            relative_path(&root.join("a/b"), &root.join("a/c")),
            std::path::Path::new("../c")
        );
    }
}
