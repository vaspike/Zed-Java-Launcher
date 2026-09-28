use crate::model::{Config, EntryKind, GroupItem, LaunchOptions, Project};
use anyhow::{bail, Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let content =
        fs::read_to_string(path).with_context(|| format!("Reading {}", path.display()))?;
    json5::from_str(&content).with_context(|| format!("Invalid JSON/JSONC in {}", path.display()))
}
pub fn load(path: &Path) -> Result<Config> {
    if !path.exists() {
        return Ok(Config::default());
    }
    let config: Config = read_json(path)?;
    if config.version != 1 {
        bail!("Unsupported config version {}", config.version);
    }
    Ok(config)
}
pub fn atomic_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path.parent().context("Output has no parent directory")?;
    fs::create_dir_all(parent)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut tmp, value)?;
    writeln!(tmp)?;
    tmp.as_file().sync_all()?;
    tmp.persist(path)
        .map_err(|e| e.error)
        .with_context(|| format!("Writing {}", path.display()))?;
    Ok(())
}
pub fn expand(root: &Path, value: &str) -> PathBuf {
    let value = value
        .replace("${workspaceFolder}", &root.to_string_lossy())
        .replace("$ZED_WORKTREE_ROOT", &root.to_string_lossy());
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}
fn arguments(value: Option<&Value>) -> Result<Vec<String>> {
    match value {
        None | Some(Value::Null) => Ok(vec![]),
        Some(Value::String(s)) => Ok(shell_words::split(s)?),
        Some(Value::Array(a)) => a
            .iter()
            .map(|s| {
                s.as_str()
                    .map(str::to_owned)
                    .context("Argument array must contain strings")
            })
            .collect(),
        _ => bail!("Arguments must be a string or array"),
    }
}

/// Imports only runnable applications, never the old plugin's invalid test launch configs.
/// Existing user options/groups win; repeated import is non-destructive.
pub fn import_vscode(project: &Project, config: &mut Config) -> Result<Vec<String>> {
    let path = project.root.join(".vscode/launch.json");
    let launch: Value = read_json(&path)?;
    let configs = launch
        .get("configurations")
        .and_then(Value::as_array)
        .context("launch.json has no configurations array")?;
    let mut warnings = vec![];
    let mut names = BTreeMap::<String, Vec<String>>::new();
    for item in configs {
        if item.get("type").and_then(Value::as_str) != Some("java")
            || item.get("request").and_then(Value::as_str) != Some("launch")
        {
            continue;
        }
        let Some(class) = item.get("mainClass").and_then(Value::as_str) else {
            continue;
        };
        let project_name = item.get("projectName").and_then(Value::as_str);
        let matches: Vec<_> = project
            .entries
            .iter()
            .filter(|e| {
                !e.kind.is_test()
                    && e.class == class
                    && project_name.is_none_or(|n| n == e.project_name)
            })
            .collect();
        if matches.len() != 1 {
            // Avoid hundreds of old test-config warnings.
            if class.ends_with("Application") {
                warnings.push(format!(
                    "Skipped stale/ambiguous application {class} (not in current Maven reactor)"
                ));
            }
            continue;
        }
        let entry = matches[0];
        if let Some(name) = item.get("name").and_then(Value::as_str) {
            names.entry(name.into()).or_default().push(entry.id.clone());
        }
        if config.entries.contains_key(&entry.id) {
            continue;
        }
        let mut options = LaunchOptions {
            vm_args: Some(arguments(item.get("vmArgs"))?),
            args: Some(arguments(item.get("args"))?),
            ..Default::default()
        };
        let args = options.vm_args.as_mut().unwrap();
        if let Some(profile) = args
            .iter()
            .rev()
            .find_map(|a| a.strip_prefix("-Dspring.profiles.active="))
        {
            options.spring_profile = Some(profile.to_owned());
        }
        args.retain(|a| !a.starts_with("-Dspring.profiles.active="));
        if let Some(env) = item.get("env") {
            options.env = serde_json::from_value(env.clone())
                .context("Invalid env in VS Code launch config")?;
        }
        options.cwd = item.get("cwd").and_then(Value::as_str).map(str::to_owned);
        if let Some(file) = item.get("envFile").and_then(Value::as_str) {
            if expand(&project.root, file).is_file() {
                options.env_file = Some(file.to_owned());
            } else if entry.kind == EntryKind::SpringBoot {
                warnings.push(format!(
                    "{}: envFile {file} does not exist; not imported",
                    entry.module
                ));
            }
        }
        for field in [
            "classPaths",
            "modulePaths",
            "javaExec",
            "preLaunchTask",
            "postDebugTask",
        ] {
            if item.get(field).is_some() {
                warnings.push(format!(
                    "{}: VS Code field {field} is not imported; review this entry before execution",
                    entry.id
                ));
            }
        }
        // Do not assume VS Code and JDTLS use identical project names in Zed.
        config.entries.insert(entry.id.clone(), options);
    }
    let group_path = project.root.join(".vscode/aggregated-launch.json");
    if group_path.exists() {
        let groups: Value = read_json(&group_path)?;
        for group in groups
            .get("configs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let name = group
                .get("name")
                .and_then(Value::as_str)
                .context("Group has no name")?;
            if config.groups.contains_key(name) {
                continue;
            }
            let mut items = vec![];
            let mut unresolved = false;
            for item in group
                .get("items")
                .and_then(Value::as_array)
                .context("Group has no items")?
            {
                let old_name = item.get("name").and_then(Value::as_str).unwrap_or("");
                match names.get(old_name).map(Vec::as_slice) {
                    Some([id]) => items.push(GroupItem {
                        entry: id.clone(),
                        enabled: item.get("enabled").and_then(Value::as_bool).unwrap_or(true),
                        delay_ms: item.get("delay").and_then(Value::as_u64).unwrap_or(0),
                    }),
                    _ => {
                        unresolved = true;
                        warnings.push(format!("Group {name}: unresolved item {old_name}; entire group left unimported"));
                    }
                }
            }
            if !unresolved {
                config.groups.insert(name.into(), items);
            }
        }
    }
    Ok(warnings)
}

pub fn environment(root: &Path, options: &LaunchOptions) -> Result<BTreeMap<String, String>> {
    let mut env = BTreeMap::new();
    if let Some(file) = &options.env_file {
        let path = expand(root, file);
        let source = fs::read_to_string(&path)
            .with_context(|| format!("Reading env file {}", path.display()))?;
        for (i, line) in source.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let line = line.strip_prefix("export ").unwrap_or(line);
            let (key, val) = line
                .split_once('=')
                .with_context(|| format!("Invalid .env assignment at line {}", i + 1))?;
            let key = key.trim();
            if key.is_empty()
                || !key.chars().enumerate().all(|(i, c)| {
                    c == '_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit())
                })
            {
                bail!("Invalid env key at line {}", i + 1);
            }
            let val = val.trim();
            // Literal dotenv values; never execute shell expansion or command substitution.
            let val = if val.starts_with('"') || val.starts_with('\'') {
                let quote = val.as_bytes()[0] as char;
                if val.len() < 2 || !val.ends_with(quote) {
                    bail!("Unterminated quoted env value at line {}", i + 1);
                }
                &val[1..val.len() - 1]
            } else {
                val
            };
            env.insert(key.into(), val.into());
        }
    }
    env.extend(options.env.clone());
    if let Some(home) = &options.java_home {
        let home = expand(root, home);
        if !home.join("bin/java").is_file() {
            bail!("java_home has no bin/java: {}", home.display());
        }
        env.insert("JAVA_HOME".into(), home.to_string_lossy().into_owned());
        let old = std::env::var_os("PATH").unwrap_or_default();
        let paths = std::iter::once(home.join("bin")).chain(std::env::split_paths(&old));
        env.insert(
            "PATH".into(),
            std::env::join_paths(paths)?.to_string_lossy().into_owned(),
        );
    }
    Ok(env)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn config_is_strict_and_jsonc_is_supported() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("c.json");
        fs::write(
            &p,
            "{ // comment\n version: 1, defaults: {spring_profile: 'local',}, }",
        )
        .unwrap();
        assert_eq!(
            load(&p).unwrap().defaults.spring_profile.as_deref(),
            Some("local")
        );
        fs::write(&p, "{defaults:{spring_profiel:'dev'}}").unwrap();
        assert!(load(&p).is_err());
    }
    #[test]
    fn dotenv_is_literal_and_config_wins() {
        let d = tempfile::tempdir().unwrap();
        fs::write(
            d.path().join(".env"),
            "export A='hello world'\nB=$(do-not-execute)\n",
        )
        .unwrap();
        let mut o = LaunchOptions {
            env_file: Some(".env".into()),
            ..Default::default()
        };
        o.env.insert("A".into(), "override".into());
        let env = environment(d.path(), &o).unwrap();
        assert_eq!(env["A"], "override");
        assert_eq!(env["B"], "$(do-not-execute)");
    }
}
