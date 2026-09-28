use crate::{
    config,
    model::{Config, EntryKind, Project},
};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::Path};

const PREFIX: &str = "[Java Launcher]";
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Manifest {
    tasks: BTreeMap<String, Value>,
    debug: BTreeMap<String, Value>,
}
#[derive(Debug, Serialize)]
pub struct Generated {
    pub tasks: Vec<Value>,
    pub debug: Vec<Value>,
    pub warnings: Vec<String>,
}
fn shell_command(binary: &Path, root: &Path, config_path: &Path, tail: &[&str]) -> String {
    let mut args = vec![
        binary.to_string_lossy().into_owned(),
        "--project".into(),
        root.to_string_lossy().into_owned(),
        "--config".into(),
        config_path.to_string_lossy().into_owned(),
    ];
    args.extend(tail.iter().map(|s| (*s).to_owned()));
    shell_words::join(args)
}
fn task(label: &str, command: String, root: &Path) -> Value {
    json!({"label":label, "command":command, "cwd":root, "use_new_terminal":false, "allow_concurrent_runs":false, "reveal":"always", "save":"all", "shell":{"with_arguments":{"program":"/bin/sh","args":["-c"]}}})
}
pub fn generate(
    project: &Project,
    config: &Config,
    config_path: &Path,
    binary: &Path,
    include_main: bool,
    include_tests: bool,
) -> Result<Generated> {
    let root = &project.root;
    let mut out = Generated {
        tasks: vec![],
        debug: vec![],
        warnings: vec![],
    };
    let command = |args: &[&str]| shell_command(binary, root, config_path, args);
    let mut refresh_args = vec!["sync-zed", "--write"];
    if include_main {
        refresh_args.push("--include-main");
    }
    if include_tests {
        refresh_args.push("--include-tests");
    }
    out.tasks.push(task(
        &format!("{PREFIX} Refresh configurations"),
        command(&refresh_args),
        root,
    ));
    out.tasks.push(task(
        &format!("{PREFIX} Running processes"),
        command(&["ps"]),
        root,
    ));
    out.tasks.push(task(
        &format!("{PREFIX} Stop all managed processes"),
        command(&["stop", "--all"]),
        root,
    ));
    for entry in &project.entries {
        let include = entry.kind == EntryKind::SpringBoot
            || (include_main && entry.kind == EntryKind::Main)
            || (include_tests && entry.kind.is_test());
        if !include {
            continue;
        }
        let label = format!(
            "{} / {}{}",
            entry.module,
            entry.class,
            entry
                .method
                .as_ref()
                .map(|m| format!("#{m}"))
                .unwrap_or_default()
        );
        out.tasks.push(task(
            &format!(
                "{PREFIX} {} {label}",
                if entry.kind.is_test() { "Test" } else { "Run" }
            ),
            command(&["run", &entry.id]),
            root,
        ));
        if entry.kind.is_test() {
            continue;
        }
        out.tasks.push(task(
            &format!("{PREFIX} Stop {label}"),
            command(&["stop", &entry.id]),
            root,
        ));
        out.tasks.push(task(
            &format!("{PREFIX} Restart {label}"),
            command(&["restart", &entry.id]),
            root,
        ));
        let options = config.options(&entry.id);
        if options.env_file.is_some() {
            out.warnings.push(format!("{}: native launch debug omitted because env_file is configured. Use `start --debug-port PORT --suspend` and an attach scenario; no secrets are copied to debug.json", entry.id));
            continue;
        }
        let cwd = options
            .cwd
            .as_ref()
            .map(|v| config::expand(root, v))
            .unwrap_or(root.clone());
        let mut debug = json!({
            "label":format!("{PREFIX} Debug {label}"), "adapter":"Java", "request":"launch", "mainClass":entry.class,
            "projectName":options.debug_project_name.as_deref().unwrap_or(&entry.project_name),
            "cwd":cwd, "vmArgs":options.jvm_args(), "args":options.args.clone().unwrap_or_default(), "env":options.env,
            "console":"integratedTerminal", "stopOnEntry":false,
            "build": {"command":command(&["prepare", &entry.id]), "cwd":root, "save":"all", "shell":{"with_arguments":{"program":"/bin/sh","args":["-c"]}}}
        });
        if let Some(home) = &options.java_home {
            debug["javaExec"] = json!(config::expand(root, home).join("bin/java"));
        }
        // Plain Java projects need the javac output directory instead of JDTLS's classpath.
        if !root.join("pom.xml").exists() {
            debug["classPaths"] = json!([crate::runtime::state_dir(root)?.join("classes")]);
            debug.as_object_mut().unwrap().remove("projectName");
        }
        out.debug.push(debug);
    }
    for (name, items) in &config.groups {
        for item in items.iter().filter(|i| i.enabled) {
            project
                .entry(&item.entry)
                .with_context(|| format!("Invalid group {name}"))?;
        }
        out.tasks.push(task(
            &format!("{PREFIX} Group up {name}"),
            command(&["group", "up", name]),
            root,
        ));
        out.tasks.push(task(
            &format!("{PREFIX} Group down {name}"),
            command(&["group", "down", name]),
            root,
        ));
    }
    out.debug.push(json!({"label":format!("{PREFIX} Attach localhost:5005"), "adapter":"Java", "request":"attach", "hostName":"127.0.0.1", "port":5005}));
    Ok(out)
}
fn label(value: &Value) -> Result<String> {
    Ok(value
        .get("label")
        .and_then(Value::as_str)
        .context("Zed entry is missing a string label; refusing to overwrite")?
        .to_owned())
}
fn merge(
    current: Vec<Value>,
    generated: &[Value],
    previous: &BTreeMap<String, Value>,
    warnings: &mut Vec<String>,
) -> Result<(Vec<Value>, BTreeMap<String, Value>)> {
    let mut current_map = BTreeMap::new();
    for v in &current {
        let key = label(v)?;
        if current_map.insert(key.clone(), v).is_some() {
            bail!("Duplicate label {key}; refusing unsafe merge");
        }
    }
    let new: BTreeMap<_, _> = generated
        .iter()
        .map(|v| Ok((label(v)?, v.clone())))
        .collect::<Result<_>>()?;
    let mut output = vec![];
    let mut tracked = BTreeMap::new();
    let mut consumed = std::collections::BTreeSet::new();
    for value in current {
        let name = label(&value)?;
        if let Some(old) = previous.get(&name) {
            if old == &value {
                if let Some(next) = new.get(&name) {
                    output.push(next.clone());
                    tracked.insert(name.clone(), next.clone());
                }
                consumed.insert(name); // unmodified stale managed entries are removed
                continue;
            }
            warnings.push(format!("Preserved manually edited entry: {name}. Move customization to launcher config to resume management."));
            // Preserve baseline, so future syncs keep recognizing user edits.
            tracked.insert(name.clone(), old.clone());
        } else if new.contains_key(&name) {
            bail!("Unmanaged entry collides with generated label {name}; rename it before sync");
        }
        consumed.insert(name);
        output.push(value);
    }
    for (name, value) in new {
        if consumed.insert(name.clone()) {
            tracked.insert(name, value.clone());
            output.push(value);
        }
    }
    Ok((output, tracked))
}
fn backup(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let backup_dir = path.parent().unwrap().join(".java-launcher-backups");
    fs::create_dir_all(&backup_dir)?;
    let mut backup = tempfile::Builder::new()
        .prefix(&format!("{}-", path.file_name().unwrap().to_string_lossy()))
        .suffix(".bak")
        .tempfile_in(backup_dir)?;
    std::io::copy(&mut fs::File::open(path)?, &mut backup)?;
    backup.keep().map_err(|e| e.error)?;
    Ok(())
}
/// Generates/merges BOTH files before writing either; each replacement is atomic.
/// JSONC formatting/comments are retained in backups, not rewritten into generated JSON.
pub fn sync(dir: &Path, generated: Generated) -> Result<Vec<String>> {
    fs::create_dir_all(dir)?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(dir.join(".java-launcher-sync.lock"))?;
    fs2::FileExt::try_lock_exclusive(&lock).context("Another sync is in progress")?;
    let manifest_path = dir.join(".java-launcher-manifest.json");
    let manifest: Manifest = if manifest_path.exists() {
        config::read_json(&manifest_path)?
    } else {
        Manifest::default()
    };
    let tasks_path = dir.join("tasks.json");
    let debug_path = dir.join("debug.json");
    let existing = |path: &Path| -> Result<Vec<Value>> {
        if path.exists() {
            config::read_json(path)
        } else {
            Ok(vec![])
        }
    };
    let old_tasks = existing(&tasks_path)?;
    let old_debug = existing(&debug_path)?;
    let mut warnings = generated.warnings;
    let (tasks, tracked_tasks) = merge(
        old_tasks.clone(),
        &generated.tasks,
        &manifest.tasks,
        &mut warnings,
    )?;
    let (debug, tracked_debug) = merge(
        old_debug.clone(),
        &generated.debug,
        &manifest.debug,
        &mut warnings,
    )?;
    // Back up before writes, including manifest for manual recovery after an interrupted sync.
    if old_tasks != tasks || old_debug != debug {
        for p in [&tasks_path, &debug_path, &manifest_path] {
            backup(p)?;
        }
    }
    if old_tasks != tasks {
        config::atomic_json(&tasks_path, &tasks)?;
    }
    if old_debug != debug {
        config::atomic_json(&debug_path, &debug)?;
    }
    config::atomic_json(
        &manifest_path,
        &Manifest {
            tasks: tracked_tasks,
            debug: tracked_debug,
        },
    )?;
    Ok(warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_user_entries_and_managed_edits() {
        let old = json!({"label":"managed", "vmArgs":"old"});
        let edited = json!({"label":"managed", "vmArgs":"mine"});
        let custom = json!({"label":"custom"});
        let mut warnings = vec![];
        let (result, _) = merge(
            vec![edited.clone(), custom.clone()],
            &[json!({"label":"managed","vmArgs":"new"})],
            &BTreeMap::from([("managed".into(), old)]),
            &mut warnings,
        )
        .unwrap();
        assert_eq!(result, vec![edited, custom]);
        assert_eq!(warnings.len(), 1);
    }
    #[test]
    fn refuses_unowned_collisions_and_removes_stale_generated() {
        let v = json!({"label":"x"});
        let mut warnings = vec![];
        assert!(merge(
            vec![v.clone()],
            &[v.clone()],
            &BTreeMap::new(),
            &mut warnings
        )
        .is_err());
        assert!(merge(
            vec![v.clone()],
            &[],
            &BTreeMap::from([("x".into(), v)]),
            &mut warnings
        )
        .unwrap()
        .0
        .is_empty());
    }
}
