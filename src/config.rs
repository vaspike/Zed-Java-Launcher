use crate::model::{Config, EntryKind, GroupItem, LaunchOptions, Project};
use anyhow::{bail, Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
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
pub fn schema_path(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("config.schema.json")
}

pub fn generate_schema(project: &Project) -> Value {
    let mut runnable_entries: Vec<(&str, String)> = project
        .entries
        .iter()
        .filter(|e| !e.kind.is_test())
        .map(|e| {
            let desc = format!(
                "{} / {} ({})",
                e.module,
                e.class.rsplit('.').next().unwrap_or(&e.class),
                match e.kind {
                    EntryKind::SpringBoot => "Spring Boot",
                    EntryKind::Main => "Main Class",
                    _ => "Runnable",
                }
            );
            (e.id.as_str(), desc)
        })
        .collect();
    runnable_entries.sort_by(|a, b| a.0.cmp(b.0));

    let entry_ids: Vec<&str> = runnable_entries.iter().map(|(id, _)| *id).collect();
    let entry_descs: Vec<&str> = runnable_entries.iter().map(|(_, desc)| desc.as_str()).collect();

    json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": "Java Launcher Configuration Schema",
        "description": "Configuration schema for .java-launcher/config.json with project-aware autocompletion",
        "type": "object",
        "properties": {
            "$schema": {
                "type": "string",
                "description": "Path or URL to JSON Schema definition"
            },
            "version": {
                "type": "integer",
                "default": 1,
                "description": "Configuration file schema version (must be 1)"
            },
            "defaults": {
                "$ref": "#/definitions/LaunchOptions",
                "description": "Default launch options inherited by all services in this project"
            },
            "entries": {
                "type": "object",
                "description": "Per-service launch options overriding defaults, keyed by service entry ID",
                "propertyNames": {
                    "enum": entry_ids
                },
                "additionalProperties": {
                    "$ref": "#/definitions/LaunchOptions"
                }
            },
            "groups": {
                "type": "object",
                "description": "Named service groups for sequential batch launching, restarting, and debugging",
                "additionalProperties": {
                    "type": "array",
                    "description": "List of services in this group, launched sequentially in array order",
                    "items": {
                        "$ref": "#/definitions/GroupItem"
                    }
                }
            }
        },
        "definitions": {
            "GroupItem": {
                "type": "object",
                "required": ["entry"],
                "properties": {
                    "entry": {
                        "type": "string",
                        "description": "Target service entry ID (e.g. module::ClassName). Select from discovered project services.",
                        "enum": entry_ids,
                        "enumDescriptions": entry_descs,
                        "markdownEnumDescriptions": entry_descs
                    },
                    "delay_ms": {
                        "type": "integer",
                        "minimum": 0,
                        "default": 0,
                        "description": "Delay in milliseconds to wait before starting the next service in the group (useful for dependency startup order)"
                    },
                    "enabled": {
                        "type": "boolean",
                        "default": true,
                        "description": "Whether this service is enabled in this group (set to false to temporarily skip launching, default: true)"
                    },
                    "name": {
                        "type": "string",
                        "description": "Optional custom human-readable display alias for this service in group logs and debug sessions"
                    }
                },
                "additionalProperties": false
            },
            "LaunchOptions": {
                "type": "object",
                "properties": {
                    "spring_profile": {
                        "type": "string",
                        "description": "Active Spring profile (sets -Dspring.profiles.active=<profile>)"
                    },
                    "vm_args": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "JVM arguments passed to the java executable (e.g. ['-Xmx512m'])"
                    },
                    "args": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Program arguments passed to the application main method"
                    },
                    "env": {
                        "type": "object",
                        "additionalProperties": { "type": "string" },
                        "description": "Environment variables passed to the child process"
                    },
                    "env_file": {
                        "type": "string",
                        "description": "Path to a .env file containing environment variables (relative to workspace root)"
                    },
                    "cwd": {
                        "type": "string",
                        "description": "Working directory for the process (supports ${workspaceFolder})"
                    },
                    "java_home": {
                        "type": "string",
                        "description": "Path to custom JDK home directory for running this service"
                    },
                    "maven_profiles": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Maven profiles to activate during compile (-P)"
                    },
                    "maven_args": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Additional Maven CLI arguments during compile"
                    },
                    "build": {
                        "type": "boolean",
                        "default": true,
                        "description": "Whether to run Maven compile before launch (default: true)"
                    },
                    "debug_project_name": {
                        "type": "string",
                        "description": "JDTLS project name override if different from Maven artifactId"
                    },
                    "terminate_on_zed_quit": {
                        "type": "boolean",
                        "description": "Whether to terminate this service automatically when the parent Zed process exits"
                    }
                },
                "additionalProperties": false
            }
        }
    })
}

pub fn write_schema(config_path: &Path, project: &Project) -> Result<PathBuf> {
    let path = schema_path(config_path);
    let schema = generate_schema(project);
    atomic_json(&path, &schema)?;
    Ok(path)
}

pub fn ensure_schema_ref(config_path: &Path) -> Result<()> {
    if !config_path.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(config_path)?;
    if !content.contains("$schema") {
        if let Some(pos) = content.find('{') {
            let mut updated = content[..=pos].to_string();
            updated.push_str("\n  \"$schema\": \"./config.schema.json\",");
            updated.push_str(&content[pos + 1..]);
            fs::write(config_path, updated)?;
        }
    }
    Ok(())
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
                        name: None,
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
    #[test]
    fn schema_generation_includes_project_entries_and_group_definitions() {
        let entry = crate::model::Entry {
            id: "warehouse::com.example.WarehouseApplication".into(),
            kind: EntryKind::SpringBoot,
            module: "warehouse".into(),
            project_name: "warehouse-service".into(),
            class: "com.example.WarehouseApplication".into(),
            method: None,
            file: "warehouse/src/main/java/com/example/WarehouseApplication.java".into(),
            line: 10,
        };
        let project = Project {
            root: PathBuf::from("/tmp/test-project"),
            modules: vec![],
            entries: vec![entry],
            warnings: vec![],
        };
        let schema = generate_schema(&project);
        assert_eq!(schema["$schema"], "http://json-schema.org/draft-07/schema#");
        let entry_enums = schema["definitions"]["GroupItem"]["properties"]["entry"]["enum"].as_array().unwrap();
        assert_eq!(entry_enums, &vec![json!("warehouse::com.example.WarehouseApplication")]);
        let entry_descs = schema["definitions"]["GroupItem"]["properties"]["entry"]["enumDescriptions"].as_array().unwrap();
        assert!(entry_descs[0].as_str().unwrap().contains("WarehouseApplication (Spring Boot)"));

        // Verify ensure_schema_ref inserts $schema when missing
        let d = tempfile::tempdir().unwrap();
        let cfg = d.path().join("config.json");
        fs::write(&cfg, "{\n  \"version\": 1\n}\n").unwrap();
        ensure_schema_ref(&cfg).unwrap();
        let content = fs::read_to_string(&cfg).unwrap();
        assert!(content.contains("\"$schema\": \"./config.schema.json\""));
    }
}

