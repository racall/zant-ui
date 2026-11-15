//! Resolve Cargo workspace inheritance before creating an isolated hot crate.
use std::path::Path;

pub(super) fn resolve(mut app: toml::Table, root: &Path) -> Result<toml::Table, String> {
    let workspace = root.ancestors().find_map(|dir| {
        let text = std::fs::read_to_string(dir.join("Cargo.toml")).ok()?;
        let manifest: toml::Table = text.parse().ok()?;
        manifest.get("workspace")?.as_table()?;
        Some((dir, manifest))
    });
    let Some((workspace_root, manifest)) = workspace else {
        return Ok(app);
    };
    let workspace = manifest["workspace"].as_table().expect("workspace");
    if let (Some(package), Some(defaults)) = (
        app.get_mut("package").and_then(toml::Value::as_table_mut),
        workspace.get("package").and_then(toml::Value::as_table),
    ) {
        for (name, value) in package.iter_mut() {
            if inherited(value) {
                *value = defaults
                    .get(name)
                    .ok_or_else(|| format!("workspace.package.{name} is missing"))?
                    .clone();
            }
        }
    }
    if let Some(deps) = app
        .get_mut("dependencies")
        .and_then(toml::Value::as_table_mut)
    {
        dependencies(deps, workspace, workspace_root)?;
    }
    if let Some(targets) = app.get_mut("target").and_then(toml::Value::as_table_mut) {
        for (_, target) in targets.iter_mut() {
            if let Some(deps) = target
                .get_mut("dependencies")
                .and_then(toml::Value::as_table_mut)
            {
                dependencies(deps, workspace, workspace_root)?;
            }
        }
    }
    if !app.contains_key("profile")
        && let Some(profile) = manifest.get("profile")
    {
        app.insert("profile".into(), profile.clone());
    }
    Ok(app)
}

fn inherited(value: &toml::Value) -> bool {
    value.get("workspace").and_then(toml::Value::as_bool) == Some(true)
}

fn dependencies(
    deps: &mut toml::Table,
    workspace: &toml::Table,
    root: &Path,
) -> Result<(), String> {
    for (name, value) in deps.iter_mut() {
        if !inherited(value) {
            continue;
        }
        let base = workspace
            .get("dependencies")
            .and_then(|deps| deps.get(name))
            .ok_or_else(|| format!("workspace.dependencies.{name} is missing"))?;
        let mut resolved = match base {
            toml::Value::String(version) => {
                toml::Table::from_iter([("version".into(), version.clone().into())])
            }
            toml::Value::Table(table) => table.clone(),
            _ => return Err(format!("invalid workspace dependency: {name}")),
        };
        if let Some(path) = resolved.get("path").and_then(toml::Value::as_str) {
            let path = root.join(path);
            resolved.insert("path".into(), path.to_string_lossy().into_owned().into());
        }
        for (key, override_value) in value.as_table().expect("workspace dependency") {
            if key == "workspace" {
                continue;
            }
            if key == "features"
                && let Some(features) = resolved.get_mut(key).and_then(toml::Value::as_array_mut)
            {
                features.extend(override_value.as_array().cloned().unwrap_or_default());
            } else {
                resolved.insert(key.clone(), override_value.clone());
            }
        }
        *value = resolved.into();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inherited_features_extend_the_workspace_features_and_paths_use_its_root() {
        let mut deps: toml::Table = "zant = { workspace = true, features = [\"app\"] }"
            .parse()
            .unwrap();
        let workspace: toml::Table =
            "[dependencies]\nzant = { path = \"crates/zant\", features = [\"base\"] }"
                .parse()
                .unwrap();
        dependencies(&mut deps, &workspace, Path::new("/repo")).unwrap();
        assert_eq!(
            deps["zant"]["path"].as_str().map(Path::new),
            Some(Path::new("/repo/crates/zant"))
        );
        assert_eq!(deps["zant"]["features"].as_array().unwrap().len(), 2);
        assert!(deps["zant"].get("workspace").is_none());
    }
}
