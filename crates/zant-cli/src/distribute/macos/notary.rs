//! Notarization and App Store Connect credentials.

use crate::distribute::{
    Asc, Ctx, Macos, Notary, expand_path, file_exists, file_or_base64, in_project, team_of,
    write_secret_file,
};
use crate::{COLOR_DIM, COLOR_RED, COLOR_RESET, fail, run_inherit};

/// Notarization credentials as ZANT_NOTARY_* variables for the script.
pub(super) fn notary_env(c: &mut Ctx, mac: &mut Macos, identity: &str) -> std::io::Result<()> {
    if mac.notary.is_none() {
        if !c.interactive {
            c.need(
                "macos.notary",
                tr!(en: "Notarization credentials: {\"profile\": \"keychain profile name\"}, {\"api_key\": true}, or {\"apple_id\", \"password\", \"team_id\"}", zh: "公证凭据：{\"profile\": \"钥匙串配置名\"}、{\"api_key\": true} 或 {\"apple_id\", \"password\", \"team_id\"}"),
            );
        }
        mac.notary = Some(ask_notary(c, identity)?);
        c.changed = true;
    }
    let notary = mac.notary.clone().unwrap_or_default();
    if let Some(profile) = &notary.profile {
        c.set_env(
            "ZANT_NOTARY_PROFILE",
            &c.value(profile, "macos.notary.profile"),
        );
        return Ok(());
    }
    if notary.api_key.unwrap_or(false) {
        let key = asc_key(c)?;
        c.set_env("ZANT_NOTARY_KEY", &key.path);
        c.set_env("ZANT_NOTARY_KEY_ID", &key.id);
        c.set_env("ZANT_NOTARY_ISSUER", &key.issuer);
        return Ok(());
    }
    if let Some(apple_id) = &notary.apple_id {
        let password = match &notary.password {
            Some(password) => password.clone(),
            None => c.need("macos.notary.password", tr!(en: "App-specific password, as $ENV_VAR_NAME", zh: "App 专用密码，写成 $环境变量名")),
        };
        c.set_env(
            "ZANT_NOTARY_APPLE_ID",
            &c.value(apple_id, "macos.notary.apple_id"),
        );
        let secret = c.secret(
            &password,
            "macos.notary.password",
            tr!(en: "App-specific password", zh: "App 专用密码"),
        );
        c.set_env("ZANT_NOTARY_PASSWORD", &secret);
        let team = match &notary.team_id {
            Some(team) => c.value(team, "macos.notary.team_id"),
            None => match team_of(identity) {
                Some(team) => team.to_string(),
                None => c.need("macos.notary.team_id", tr!(en: "Team ID", zh: "团队 ID")),
            },
        };
        c.set_env("ZANT_NOTARY_TEAM", &team);
        return Ok(());
    }
    fail(
        tr!(en: "macos.notary requires profile, api_key, or apple_id", zh: "macos.notary 要设置 profile、api_key 或 apple_id 其中一种"),
    )
}

fn ask_notary(c: &mut Ctx, identity: &str) -> std::io::Result<Notary> {
    let index = c.choose(
        tr!(en: "Apple account for notarization", zh: "公证用的苹果帐户"),
        &[
            tr!(en: "Existing notarytool profile in Keychain (enter name)", zh: "已经用 notarytool 存在钥匙串里的配置（输入名称）"),
            tr!(en: "Apple ID + app-specific password (verified by notarytool and saved in Keychain, outside the configuration file)", zh: "Apple ID + App 专用密码（notarytool 验证后存进钥匙串，不写进配置文件）"),
            tr!(en: "App Store Connect API key (Issuer ID, Key ID, .p8 file)", zh: "App Store Connect API key（Issuer ID、Key ID、.p8 文件）"),
        ],
        1,
    );
    match index {
        0 => Ok(Notary {
            profile: Some(c.ask_required(
                tr!(en: "Keychain profile name", zh: "钥匙串配置名称"),
                "zant-notary",
            )),
            ..Notary::default()
        }),
        1 => {
            let apple_id = c.ask_required(tr!(en: "Apple ID (email)", zh: "Apple ID（邮箱）"), "");
            let team = c.ask_required(
                tr!(en: "Team ID", zh: "团队 ID"),
                team_of(identity).unwrap_or(""),
            );
            let name = c.ask_required(
                tr!(en: "Profile name to save in Keychain", zh: "存进钥匙串的配置名称"),
                "zant-notary",
            );
            eprintln!(
                "{}",
                tr!(en: "  {COLOR_DIM}Generate an app-specific password at account.apple.com → Sign-In and Security → App-Specific Passwords; notarytool will prompt for it below, and zant cannot see it{COLOR_RESET}", zh: "  {COLOR_DIM}App 专用密码在 account.apple.com → 登录与安全 → App 专用密码 生成；下面由 notarytool 询问，zant 看不到它{COLOR_RESET}",
             COLOR_DIM = COLOR_DIM, COLOR_RESET = COLOR_RESET)
            );
            run_inherit(
                &[
                    "xcrun".to_string(),
                    "notarytool".to_string(),
                    "store-credentials".to_string(),
                    name.clone(),
                    "--apple-id".to_string(),
                    apple_id,
                    "--team-id".to_string(),
                    team,
                ],
                &c.root.clone(),
            );
            Ok(Notary {
                profile: Some(name),
                ..Notary::default()
            })
        }
        _ => {
            let _ = asc_key(c)?;
            Ok(Notary {
                api_key: Some(true),
                ..Notary::default()
            })
        }
    }
}

/// The App Store Connect API key, asked once (Issuer ID, Key ID, .p8 path).
pub(super) fn asc_key(c: &mut Ctx) -> std::io::Result<Asc> {
    if let Some(key) = &c.asc {
        return Ok(key.clone());
    }
    let mut key = c.file.app_store_connect.take().unwrap_or_default();
    if key.issuer_id.is_none() || key.key_id.is_none() || key.key.is_none() {
        if !c.interactive {
            c.need(
                "app_store_connect",
                tr!(en: "issuer_id, key_id, key (.p8 file path or content, as $ENV_VAR_NAME)", zh: "issuer_id、key_id、key（.p8 文件路径或内容，写成 $环境变量名）"),
            );
        }
        eprintln!(
            "{}",
            tr!(en: "\nApp Store Connect API key (App Store Connect → Users and Access → Integrations → App Store Connect API; “App Manager” permission)", zh: "\nApp Store Connect API key（App Store Connect → 用户和访问 → 集成 → App Store Connect API，权限「App 管理」）",
            )
        );
        if key.issuer_id.is_none() {
            key.issuer_id = Some(c.ask_required("Issuer ID", ""));
        }
        if key.key_id.is_none() {
            key.key_id = Some(c.ask_required("Key ID", ""));
        }
        if key.key.is_none() {
            let home = c.get_env("HOME").unwrap_or_default();
            let key_id = key.key_id.clone().unwrap_or_default();
            let mut default = String::new();
            for sub in [
                ".appstoreconnect/private_keys",
                "private_keys",
                ".private_keys",
            ] {
                let path = format!("{home}/{sub}/AuthKey_{key_id}.p8");
                if file_exists(c, &path) {
                    default = format!("~/{sub}/AuthKey_{key_id}.p8");
                }
            }
            loop {
                let path = c.ask_required(tr!(en: ".p8 file path", zh: ".p8 文件路径"), &default);
                let full = expand_path(c, &path);
                if !file_exists(c, &full) {
                    eprintln!("{}", tr!(en: "  File not found", zh: "  找不到这个文件",));
                    continue;
                }
                if in_project(c, &full) {
                    eprintln!(
                        "{}",
                        tr!(en: "  {COLOR_RED}Warning{COLOR_RESET}: this .p8 file is inside the project; do not commit it (move it to ~/.appstoreconnect/private_keys/)", zh: "  {COLOR_RED}注意{COLOR_RESET}：这个 .p8 在项目目录里，千万不要提交到仓库（建议移到 ~/.appstoreconnect/private_keys/）",
                     COLOR_RED = COLOR_RED, COLOR_RESET = COLOR_RESET)
                    );
                }
                key.key = Some(path);
                break;
            }
        }
        c.changed = true;
    }
    let issuer = c.value(
        key.issuer_id.as_deref().unwrap(),
        "app_store_connect.issuer_id",
    );
    let id = c.value(key.key_id.as_deref().unwrap(), "app_store_connect.key_id");
    let source = c.value(key.key.as_deref().unwrap(), "app_store_connect.key");
    let name = format!("AuthKey_{id}.p8");
    let path = if source.contains("-----BEGIN") {
        write_secret_file(c, &name, source.as_bytes())?
    } else {
        file_or_base64(c, &source, "app_store_connect.key", &name)?
    };
    let asc = Asc { issuer, id, path };
    c.asc = Some(asc.clone());
    c.file.app_store_connect = Some(key);
    Ok(asc)
}
