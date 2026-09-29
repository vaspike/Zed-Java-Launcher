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
    if include_tests {
        refresh_args.push("--include-tests");
    }
    out.tasks.push(task(
        &format!("{PREFIX} Refresh configurations"),
        command(&refresh_args),
        root,
    ));
    let mut refresh_main_args = vec!["sync-zed", "--include-main", "--write"];
    if include_tests {
        refresh_main_args.push("--include-tests");
    }
    out.tasks.push(task(
        &format!("{PREFIX} Refresh configurations (include main)"),
        command(&refresh_main_args),
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
        if include_main || include_tests {
            out.tasks.push(task(
                &format!(
                    "{PREFIX} {} {label}",
                    if entry.kind.is_test() { "Test" } else { "Run" }
                ),
                command(&["run", &entry.id]),
                root,
            ));
            if !entry.kind.is_test() {
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
            }
        }
        if entry.kind.is_test() {
            continue;
        }
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
        let short_name = entry
            .class
            .rsplit('.')
            .next()
            .unwrap_or(&entry.class);
        let is_duplicate = project
            .entries
            .iter()
            .filter(|e| {
                (e.kind == EntryKind::SpringBoot || (include_main && e.kind == EntryKind::Main))
                    && e.class.rsplit('.').next() == Some(short_name)
            })
            .count()
            > 1;
        let debug_label = if is_duplicate {
            format!("JL-{}-{}", entry.module, short_name)
        } else {
            format!("JL-{}", short_name)
        };
        let mut debug = json!({
            "label": debug_label,
            "name": short_name,
            "adapter": "java-launcher",
            "request": "launch",
            "entry": entry.id,
            "mainClass": entry.class,
            "projectName": options.debug_project_name.as_deref().unwrap_or(&entry.project_name),
            "cwd": cwd,
            "vmArgs": options.jvm_args(),
            "args": options.args.clone().unwrap_or_default(),
            "env": options.env,
            "console": "integratedTerminal",
            "stopOnEntry": false,
        });
        if let Some(home) = &options.java_home {
            debug["javaExec"] = json!(config::expand(root, home).join("bin/java"));
        }
        // Plain Java projects need the javac output directory instead of JDTLS's classpath.
        if !root.join("pom.xml").exists() {
            debug["classPaths"] = json!([crate::runtime::state_dir(root)?.join("classes")]);
            debug["build"] = json!({"command":command(&["prepare", &entry.id]), "cwd":root, "save":"all", "shell":{"with_arguments":{"program":"/bin/sh","args":["-c"]}}});
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
        out.debug.push(json!({
            "label": format!("JL-Group-{name}"),
            "adapter": "java-launcher",
            "request": "launch",
            "group": name,
        }));
        out.tasks.push(task(
            &format!("{PREFIX} Group up {name}"),
            command(&["group", "up", name]),
            root,
        ));
        out.tasks.push(task(
            &format!("{PREFIX} Group restart {name}"),
            command(&["group", "restart", name]),
            root,
        ));
        out.tasks.push(task(
            &format!("{PREFIX} Group down {name}"),
            command(&["group", "down", name]),
            root,
        ));
    }
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

fn merge_debug(
    current: Vec<Value>,
    generated: &[Value],
    project: &Project,
    config: &Config,
    warnings: &mut Vec<String>,
) -> Result<(Vec<Value>, BTreeMap<String, Value>)> {
    let mut output = Vec::new();
    let mut seen_labels = std::collections::BTreeSet::new();

    // Track which services and groups already have debug configs in current
    let mut covered_entries = std::collections::BTreeSet::new();
    let mut covered_groups = std::collections::BTreeSet::new();

    // 1. Process existing debug configurations in `current`
    for item in current {
        let label_str = match label(&item) {
            Ok(l) => l,
            Err(_) => continue,
        };

        if !seen_labels.insert(label_str.clone()) {
            warnings.push(format!("Skipped duplicate debug configuration label: {}", label_str));
            continue;
        }

        let is_attach = item
            .get("request")
            .and_then(Value::as_str)
            .map(|r| r == "attach")
            .unwrap_or(false)
            || label_str.to_lowercase().contains("attach");

        // Rule 1: Never delete attach configs
        if is_attach {
            output.push(item);
            continue;
        }

        // Rule 2: Group configuration
        if let Some(group_name) = item.get("group").and_then(Value::as_str) {
            if config.groups.contains_key(group_name) {
                // Group still exists -> preserve existing config and its label!
                covered_groups.insert(group_name.to_string());
                output.push(item);
            } else {
                // Group was deleted from config -> remove it
                warnings.push(format!("Removed debug config for deleted group: {}", group_name));
            }
            continue;
        }

        // Rule 3: Service/class launch configuration
        let entry_id = item.get("entry").and_then(Value::as_str);
        let main_class = item.get("mainClass").and_then(Value::as_str);

        if entry_id.is_some() || main_class.is_some() {
            // Find if this entry/class still exists in project.entries
            let matched_entry = project.entries.iter().find(|e| {
                if let Some(id) = entry_id {
                    if e.id == id {
                        return true;
                    }
                }
                if let Some(mc) = main_class {
                    if e.class == mc {
                        return true;
                    }
                }
                false
            });

            if let Some(e) = matched_entry {
                // Service still exists -> preserve existing config and its label!
                covered_entries.insert(e.id.clone());
                output.push(item);
            } else {
                // Service class was deleted/renamed -> remove it
                let name = entry_id.or(main_class).unwrap_or(&label_str);
                warnings.push(format!("Removed debug config for deleted service: {}", name));
            }
            continue;
        }

        // Rule 4: Any other user-defined configuration: preserve as-is!
        output.push(item);
    }

    // 2. Incremental addition: Add newly generated configs that are not yet covered
    for gen in generated {
        // If it's a group config
        if let Some(group_name) = gen.get("group").and_then(Value::as_str) {
            if !covered_groups.contains(group_name) {
                let mut l = label(gen)?;
                let mut counter = 2;
                while !seen_labels.insert(l.clone()) {
                    l = format!("{}-{}", label(gen)?, counter);
                    counter += 1;
                }
                let mut item = gen.clone();
                item["label"] = json!(l);
                covered_groups.insert(group_name.to_string());
                output.push(item);
            }
            continue;
        }

        // If it's a service config
        let gen_entry = gen.get("entry").and_then(Value::as_str);
        let gen_class = gen.get("mainClass").and_then(Value::as_str);

        let already_covered = project.entries.iter().any(|e| {
            let matches = (gen_entry.is_some() && gen_entry == Some(&e.id))
                || (gen_class.is_some() && gen_class == Some(&e.class));
            matches && covered_entries.contains(&e.id)
        });

        if !already_covered {
            let mut l = label(gen)?;
            let mut counter = 2;
            while !seen_labels.insert(l.clone()) {
                l = format!("{}-{}", label(gen)?, counter);
                counter += 1;
            }
            let mut item = gen.clone();
            item["label"] = json!(l);
            if let Some(id) = gen_entry {
                covered_entries.insert(id.to_string());
            }
            output.push(item);
        }
    }

    let mut tracked = BTreeMap::new();
    for item in &output {
        if let Ok(l) = label(item) {
            tracked.insert(l, item.clone());
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
pub fn sync(
    dir: &Path,
    generated: Generated,
    project: &Project,
    config: &Config,
) -> Result<Vec<String>> {
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
    let (debug, tracked_debug) = merge_debug(
        old_debug.clone(),
        &generated.debug,
        project,
        config,
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
    use crate::model::Entry;
    use std::path::PathBuf;
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

    #[test]
    fn test_simplified_jl_labels_and_no_attach_5005() {
        let entry = Entry {
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
        let mut config = Config::default();
        config.groups.insert(
            "uis".into(),
            vec![crate::model::GroupItem {
                entry: "warehouse::com.example.WarehouseApplication".into(),
                name: Some("WarehouseApplication".into()),
                enabled: true,
                delay_ms: 0,
            }],
        );
        let config_path = PathBuf::from("/tmp/test-project/.java-launcher/config.json");
        let binary = PathBuf::from("/tmp/bin/java-launcher");
        let gen = generate(&project, &config, &config_path, &binary, false, false).unwrap();

        // 1. Verify single service debug label is JL-WarehouseApplication
        let single_debug = gen.debug.iter().find(|d| d["label"] == "JL-WarehouseApplication");
        assert!(single_debug.is_some(), "Expected JL-WarehouseApplication in debug configs");
        let single = single_debug.unwrap();
        assert_eq!(single["adapter"], "java-launcher");
        assert_eq!(single["name"], "WarehouseApplication");

        // 2. Verify group debug label is JL-Group-uis
        let group_debug = gen.debug.iter().find(|d| d["label"] == "JL-Group-uis");
        assert!(group_debug.is_some(), "Expected JL-Group-uis in debug configs");
        let group = group_debug.unwrap();
        assert_eq!(group["adapter"], "java-launcher");
        assert_eq!(group["group"], "uis");

        // 3. Verify Attach localhost:5005 is NOT generated
        let attach = gen.debug.iter().find(|d| d["label"].as_str().unwrap_or("").contains("5005"));
        assert!(attach.is_none(), "Attach 5005 must not be generated");

        // 4. Verify Group tasks (up, restart, down)
        assert!(gen.tasks.iter().any(|t| t["label"] == "[Java Launcher] Group up uis"));
        assert!(gen.tasks.iter().any(|t| t["label"] == "[Java Launcher] Group restart uis"));
        assert!(gen.tasks.iter().any(|t| t["label"] == "[Java Launcher] Group down uis"));

        // 5. Verify Refresh configurations tasks
        assert!(gen.tasks.iter().any(|t| t["label"] == "[Java Launcher] Refresh configurations"));
        assert!(gen.tasks.iter().any(|t| t["label"] == "[Java Launcher] Refresh configurations (include main)"));
    }

    #[test]
    fn test_merge_debug_preserves_saved_labels_and_attaches_and_handles_incremental() {
        let entry1 = Entry {
            id: "warehouse::com.example.WarehouseApplication".into(),
            kind: EntryKind::SpringBoot,
            module: "warehouse".into(),
            project_name: "warehouse-service".into(),
            class: "com.example.WarehouseApplication".into(),
            method: None,
            file: "warehouse/src/main/java/com/example/WarehouseApplication.java".into(),
            line: 10,
        };
        let entry2 = Entry {
            id: "websocket::com.example.WebsocketApplication".into(),
            kind: EntryKind::SpringBoot,
            module: "websocket".into(),
            project_name: "websocket-service".into(),
            class: "com.example.WebsocketApplication".into(),
            method: None,
            file: "websocket/src/main/java/com/example/WebsocketApplication.java".into(),
            line: 10,
        };
        let project = Project {
            root: PathBuf::from("/tmp/test-project"),
            modules: vec![],
            entries: vec![entry1, entry2],
            warnings: vec![],
        };
        let mut config = Config::default();
        config.groups.insert(
            "uis".into(),
            vec![],
        );

        // Existing debug.json has:
        // 1. Existing service with custom / older label "[Java Launcher] Debug WarehouseApplication"
        // 2. An attach configuration "[Java Launcher] Attach localhost:5005"
        // 3. A custom attach configuration "Attach to remote JVM"
        // 4. A service that was deleted from the codebase "com.example.DeletedApplication"
        // 5. A group that was deleted from config "old_group"
        let current = vec![
            json!({
                "label": "[Java Launcher] Debug WarehouseApplication",
                "adapter": "Java",
                "request": "launch",
                "mainClass": "com.example.WarehouseApplication",
                "vmArgs": "-Xmx1024m"
            }),
            json!({
                "label": "[Java Launcher] Attach localhost:5005",
                "adapter": "Java",
                "request": "attach",
                "hostName": "127.0.0.1",
                "port": 5005
            }),
            json!({
                "label": "Attach to remote JVM",
                "adapter": "Java",
                "request": "attach",
                "port": 5006
            }),
            json!({
                "label": "JL-DeletedApplication",
                "adapter": "java-launcher",
                "request": "launch",
                "mainClass": "com.example.DeletedApplication"
            }),
            json!({
                "label": "JL-Group-old_group",
                "adapter": "java-launcher",
                "request": "launch",
                "group": "old_group"
            }),
        ];

        // Newly generated configs:
        // 1. JL-WarehouseApplication (already covered by existing config!)
        // 2. JL-WebsocketApplication (NEW service!)
        // 3. JL-Group-uis (NEW group!)
        let generated = vec![
            json!({
                "label": "JL-WarehouseApplication",
                "name": "WarehouseApplication",
                "adapter": "java-launcher",
                "request": "launch",
                "entry": "warehouse::com.example.WarehouseApplication",
                "mainClass": "com.example.WarehouseApplication"
            }),
            json!({
                "label": "JL-WebsocketApplication",
                "name": "WebsocketApplication",
                "adapter": "java-launcher",
                "request": "launch",
                "entry": "websocket::com.example.WebsocketApplication",
                "mainClass": "com.example.WebsocketApplication"
            }),
            json!({
                "label": "JL-Group-uis",
                "adapter": "java-launcher",
                "request": "launch",
                "group": "uis"
            }),
        ];

        let mut warnings = vec![];
        let (merged, _) = merge_debug(current, &generated, &project, &config, &mut warnings).unwrap();

        let labels: Vec<String> = merged.iter().map(|item| item["label"].as_str().unwrap().to_string()).collect();

        // 1. Existing service label MUST NOT be modified!
        assert!(labels.contains(&"[Java Launcher] Debug WarehouseApplication".to_string()));
        assert!(!labels.contains(&"JL-WarehouseApplication".to_string()), "Must not replace existing label with new label");
        let warehouse = merged.iter().find(|i| i["label"] == "[Java Launcher] Debug WarehouseApplication").unwrap();
        assert_eq!(warehouse["vmArgs"], "-Xmx1024m", "Preserves existing configuration contents");

        // 2. Attach configurations MUST NOT be deleted!
        assert!(labels.contains(&"[Java Launcher] Attach localhost:5005".to_string()));
        assert!(labels.contains(&"Attach to remote JVM".to_string()));

        // 3. Newly discovered service MUST be incrementally added!
        assert!(labels.contains(&"JL-WebsocketApplication".to_string()));

        // 4. Newly discovered group MUST be incrementally added!
        assert!(labels.contains(&"JL-Group-uis".to_string()));

        // 5. Deleted service MUST be removed!
        assert!(!labels.contains(&"JL-DeletedApplication".to_string()));

        // 6. Deleted group MUST be removed!
        assert!(!labels.contains(&"JL-Group-old_group".to_string()));
    }
}
