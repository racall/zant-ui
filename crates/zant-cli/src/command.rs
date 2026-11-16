//! Argument parsing, translated help, and command dispatch.

use crate::{create, distribute, fail, hot, lang, run, update};
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};

pub fn usage() -> &'static str {
    tr!(en: "Usage: zant [--lang en|zh] <command>\n  create <name>         Create a project (TSX + Rust)\n  run                   Build and run with hot reload (--no-hot disables it; --watch restarts)\n  compile [--release]   Compile only\n  package               Create a local .app / application directory\n  build [config]        Release: sign, notarize, package, or submit to a store\n                        (reads build.config.json; --yes skips prompts for CI)\n  sync                  Regenerate configuration code\n  update                Update Zant UI and the current project's dependencies\n                        (source checkout: git pull and rebuild zant)\n\n  --lang en|zh          Message language (default: en; overrides ZANT_LANG)\n  ZANT_LANG             Set the message language to en or zh\n\n", zh: "用法: zant <命令>\n  create <项目名>       生成项目（TSX + Rust）\n  run                   编译并运行，改动后界面原地刷新（--no-hot 关闭，--watch 改为重启）\n  compile [--release]   只编译\n  package               生成本机测试用的 .app / 程序目录\n  build [配置文件]      发布构建：签名、公证、DMG / 安装包，或提交商店\n                        （默认读 build.config.json；--yes 不提问，CI 用）\n  sync                  重新生成配置代码\n  update                更新到最新发布版，并把当前项目的依赖改到新版本\n                        （源码仓库：git pull 并重新编译 zant）\n\n  --lang en|zh          提示语言（默认 en；优先于 ZANT_LANG）\n  ZANT_LANG             设置提示语言为 en 或 zh\n\n")
}

// ---- command line ----

#[derive(Parser)]
#[command(
    name = "zant",
    version,
    about = tr!(en: "Zant UI CLI: create, run, and package apps", zh: "Zant UI 命令行工具：创建、运行、打包应用"),
    disable_help_subcommand = true
)]
struct Cli {
    #[arg(long = "lang", global = true, value_name = "en|zh", help = tr!(en: "Message language (default: en; overrides ZANT_LANG)", zh: "提示语言（默认 en；优先于 ZANT_LANG）"))]
    _lang: Option<String>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    #[command(about = tr!(en: "Create a project", zh: "生成项目"))]
    Create {
        #[arg(help = tr!(en: "Project name (starts with a letter; letters, digits, - and _ only)", zh: "项目名（只能以字母开头，包含字母、数字、-、_）"))]
        name: String,
        #[arg(help = tr!(en: "Use defaults without prompting", zh: "不提问，使用默认值"))]
        #[arg(long)]
        yes: bool,
        #[arg(help = tr!(en: "Overwrite an existing directory", zh: "覆盖已存在的目录"))]
        #[arg(long)]
        force: bool,
        #[arg(help = tr!(en: "App title", zh: "应用标题"))]
        #[arg(long)]
        title: Option<String>,
        #[arg(help = tr!(en: "Window style: native, hidden-titlebar, or borderless", zh: "窗口风格：native、hidden-titlebar 或 borderless"))]
        #[arg(long)]
        style: Option<String>,
        #[arg(help = tr!(en: "App identifier (reverse domain, e.g. com.xx.test)", zh: "应用标识（反向域名，如 com.xx.test）"))]
        #[arg(long = "bundle-id")]
        bundle_id: Option<String>,
        #[arg(help = tr!(en: "macOS sandbox: true or false", zh: "macOS 沙盒：true 或 false"))]
        #[arg(long, value_name = "true|false")]
        sandbox: Option<String>,
        #[arg(help = tr!(en: "macOS signing certificate", zh: "macOS 签名证书"))]
        #[arg(long)]
        sign: Option<String>,
    },
    #[command(about = tr!(en: "Build and run", zh: "编译并运行"))]
    Run {
        #[arg(help = tr!(en: "Rebuild and restart the app after changes", zh: "改动后重新编译并重启应用（不保留窗口）"))]
        #[arg(long)]
        watch: bool,
        #[arg(help = tr!(en: "Run once without watching for changes", zh: "只运行一次，不监听改动"))]
        #[arg(long = "no-hot")]
        no_hot: bool,
        #[arg(help = tr!(en: "Release build", zh: "Release 构建"))]
        #[arg(long)]
        release: bool,
    },
    #[command(about = tr!(en: "Compile only", zh: "只编译"))]
    Compile {
        #[arg(help = tr!(en: "Release build", zh: "Release 构建"))]
        #[arg(long)]
        release: bool,
    },
    #[command(about = tr!(en: "Create a local .app / application directory", zh: "生成本机测试用的 .app / 程序目录"))]
    Package,
    #[command(about = tr!(en: "Release: sign, notarize, create a DMG / installer, or submit to a store", zh: "发布构建：签名、公证、DMG / 安装包，或提交商店"))]
    Build {
        #[arg(help = tr!(en: "Configuration file (default: build.config.json)", zh: "配置文件（默认 build.config.json）"))]
        file: Option<String>,
        #[arg(help = tr!(en: "Skip prompts (for CI)", zh: "不提问（CI 用）"))]
        #[arg(long, short = 'y')]
        yes: bool,
        #[arg(help = tr!(en: "Legacy flag; use zant compile instead", zh: "旧用法（zant build --release），现在会提示改用 zant compile"))]
        #[arg(long, hide = true)]
        release: bool,
    },
    #[command(about = tr!(en: "Regenerate configuration code", zh: "重新生成配置代码"))]
    Sync,
    #[command(about = tr!(en: "Update Zant UI and project dependencies (source checkout: git pull and rebuild)", zh: "更新到最新发布版，并把当前项目的依赖改到新版本（源码仓库：git pull 并重新编译）"))]
    Update,
}

fn localized_command(mut command: clap::Command) -> clap::Command {
    command = command
        .help_template(tr!(en: "{about-with-newline}\nUsage: {usage}\n\n{all-args}{after-help}", zh: "{about-with-newline}\n用法: {usage}\n\n{all-args}{after-help}"))
        .subcommand_help_heading(tr!(en: "Commands", zh: "命令"))
        .next_help_heading(tr!(en: "Options", zh: "选项"))
        .disable_help_flag(true)
        .arg(clap::Arg::new("help").long("help").short('h').action(clap::ArgAction::Help).help(tr!(en: "Print help", zh: "显示帮助")));
    if command.get_version().is_some() {
        command = command.disable_version_flag(true).arg(
            clap::Arg::new("version")
                .long("version")
                .short('V')
                .action(clap::ArgAction::Version)
                .help(tr!(en: "Print version", zh: "显示版本")),
        );
    }
    command = command.mut_args(|arg| {
        let positional = arg.is_positional();
        arg.help_heading(if positional {
            tr!(en: "Arguments", zh: "参数")
        } else {
            tr!(en: "Options", zh: "选项")
        })
    });
    let names: Vec<String> = command
        .get_subcommands()
        .map(|sub| sub.get_name().to_string())
        .collect();
    for name in names {
        command = command.mut_subcommand(name, localized_command);
    }
    command
}

pub fn run() {
    let env = std::env::var("ZANT_LANG").ok();
    let (args, chinese) = lang::extract(std::env::args().collect(), env.as_deref())
        .unwrap_or_else(|error| fail(error));
    lang::init(chinese);
    let matches = localized_command(Cli::command()).get_matches_from(args);
    let cli = Cli::from_arg_matches(&matches).expect("valid CLI matches");
    let Some(command) = cli.command else {
        eprint!("{}", usage());
        return;
    };
    let result: std::io::Result<()> = match command {
        Command::Create {
            name,
            yes,
            force,
            title,
            style,
            bundle_id,
            sandbox,
            sign,
        } => create::cmd_create(create::CreateOptions {
            name,
            yes,
            force,
            title,
            style,
            bundle_id,
            sandbox,
            sign,
        }),
        Command::Run {
            watch,
            no_hot,
            release,
        } => {
            if watch {
                run::cmd_watch(release)
            } else if no_hot || !hot::available() {
                run::cmd_run(release)
            } else {
                hot::cmd_hot(release)
            }
        }
        Command::Compile { release } => run::cmd_compile(release),
        Command::Package => run::cmd_package(),
        Command::Build { file, yes, release } => {
            if release {
                fail(
                    tr!(en: "zant build creates releases (signing, packaging, notarization, or store submission); use zant compile [--release] to compile only", zh: "zant build 现在是发布构建（签名、打包、公证或提交商店）；只编译请用 zant compile [--release]"),
                );
            }
            distribute::cmd_build(file, yes)
        }
        Command::Sync => run::cmd_sync(),
        Command::Update => update::cmd_update(),
    };
    if let Err(err) = result {
        eprintln!("zant: {err}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn usage_lists_every_command() {
        for needle in [
            "create <name>",
            "run ",
            "compile [--release]",
            "package",
            "build [config]",
            "sync",
            "update",
        ] {
            assert!(usage().contains(needle), "{needle} missing from usage");
        }
    }
}
