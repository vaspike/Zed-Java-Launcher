use crate::{
    config,
    model::{Entry, LaunchOptions, Project},
    runtime,
};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReactorModule {
    pub artifact_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    pub classes_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub entry: String,
    pub prepare: Vec<CommandSpec>,
    pub launch: CommandSpec,
    pub classpath_file: Option<PathBuf>,
    pub classes: Vec<PathBuf>,
    pub notes: Vec<String>,
    pub environment_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reactor_modules: Vec<ReactorModule>,
}
fn tool(options: &LaunchOptions, root: &Path, name: &str) -> String {
    options
        .java_home
        .as_ref()
        .map(|h| {
            config::expand(root, h)
                .join("bin")
                .join(name)
                .to_string_lossy()
                .into_owned()
        })
        .unwrap_or_else(|| name.into())
}
fn maven(root: &Path) -> String {
    // A wrapper without its distribution properties is not usable (UIS has this layout).
    if root.join("mvnw").is_file() && root.join(".mvn/wrapper/maven-wrapper.properties").is_file() {
        root.join("mvnw").to_string_lossy().into_owned()
    } else {
        "mvn".into()
    }
}
pub fn build(
    project: &Project,
    entry: &Entry,
    options: &LaunchOptions,
    debug_port: Option<u16>,
    suspend: bool,
) -> Result<Plan> {
    if debug_port == Some(0) {
        bail!("Debug port must be nonzero");
    }
    if entry.kind.is_test() && debug_port.is_some() {
        bail!("Test debugging is not yet supported");
    }
    let root = &project.root;
    let state = runtime::state_dir(root)?;
    let cwd = options
        .cwd
        .as_ref()
        .map(|p| config::expand(root, p))
        .unwrap_or(root.clone());
    if !cwd.is_dir() {
        bail!("Working directory does not exist: {}", cwd.display());
    }
    let mut plan = Plan {
        entry: entry.id.clone(),
        prepare: vec![],
        launch: CommandSpec {
            program: tool(options, root, "java"),
            args: vec![],
            cwd,
        },
        classpath_file: None,
        classes: vec![],
        notes: vec![],
        environment_keys: options.env.keys().cloned().collect(),
        reactor_modules: vec![],
    };
    if options.env_file.is_some() {
        plan.notes.push(
            "env_file is loaded literally at runtime; values are not displayed in plans".into(),
        );
    }
    let maven_project = root.join("pom.xml").exists();
    if maven_project {
        let program = maven(root);
        let mut base = vec!["-B".into()];
        if !options
            .maven_args
            .as_ref()
            .map_or(false, |args| args.iter().any(|a| a.contains("antrun.skip")))
        {
            base.push("-Dmaven.antrun.skip=true".into());
        }
        if let Some(extra) = &options.maven_args {
            base.extend(extra.iter().cloned());
        }
        if let Some(profiles) = &options.maven_profiles {
            if !profiles.is_empty() {
                base.push(format!("-P{}", profiles.join(",")));
            }
        }
        let selected = if entry.module == "." {
            vec![]
        } else {
            vec!["-pl".into(), entry.module.clone()]
        };
        if options.build.unwrap_or(true) {
            let mut args = base.clone();
            args.extend(selected.clone());
            if entry.module != "." {
                args.push("-am".into());
            }
            args.extend(["test-compile".into(), "-DskipTests".into()]);
            plan.prepare.push(CommandSpec {
                program: program.clone(),
                args,
                cwd: root.clone(),
            });
            plan.notes.push("Maven test-compile builds the selected module and upstream dependencies; package phase plugins are not executed".into());

            // Collect upstream reactor candidate modules for in-place classpath replacement
            for module in &project.modules {
                if module.path == entry.module || module.packaging == "pom" {
                    continue;
                }
                plan.reactor_modules.push(ReactorModule {
                    artifact_id: module.artifact_id.clone(),
                    group_id: module.group_id.clone(),
                    classes_dir: root.join(&module.path).join("target/classes"),
                });
            }
        }
        if entry.kind.is_test() {
            let selector = if let Some(method) = &entry.method {
                format!("{}#{method}", entry.class)
            } else {
                entry.class.clone()
            };
            let mut args = base;
            args.extend(selected);
            args.extend(["test".into(), format!("-Dtest={selector}")]);
            let jvm = options.jvm_args();
            if !jvm.is_empty() {
                args.push(format!("-DargLine={}", shell_words::join(jvm)));
            }
            plan.launch = CommandSpec {
                program,
                args,
                cwd: root.clone(),
            };
            if options.args.as_ref().is_some_and(|a| !a.is_empty()) {
                bail!("Program args are not applicable to Maven tests");
            }
            return Ok(plan);
        }
        let cp = state.join(format!("{}.classpath", runtime::hash(&entry.id)));
        let mut args = base;
        args.extend(selected);
        if entry.module != "." {
            // Keep upstream modules in the reactor. UIS installs module POMs whose
            // parent version still contains ${uis.version}; resolving those from
            // the local repository in a second standalone invocation fails.
            args.push("-am".into());
        }
        args.extend([
            "org.apache.maven.plugins:maven-dependency-plugin:3.8.1:build-classpath".into(),
            "-DincludeScope=runtime".into(),
            format!("-Dmdep.outputFile={}", cp.display()),
        ]);
        plan.prepare.push(CommandSpec {
            program,
            args,
            cwd: root.clone(),
        });
        plan.classpath_file = Some(cp);
        plan.classes
            .push(root.join(&entry.module).join("target/classes"));
    } else {
        if entry.kind.is_test() {
            bail!("Tests require a Maven project in v0.1");
        }
        let classes = state.join("classes");
        plan.classes.push(classes.clone());
        if options.build.unwrap_or(true) {
            let mut args = vec![
                "-encoding".into(),
                "UTF-8".into(),
                "-d".into(),
                classes.to_string_lossy().into_owned(),
            ];
            for file in WalkDir::new(root)
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| {
                    !matches!(
                        e.file_name().to_str(),
                        Some(
                            ".git"
                                | "target"
                                | "build"
                                | "out"
                                | "bin"
                                | ".java-launcher"
                                | "node_modules"
                        )
                    )
                })
            {
                let file = file?;
                if file.file_type().is_file()
                    && file.path().extension().is_some_and(|e| e == "java")
                {
                    args.push(file.path().to_string_lossy().into_owned());
                }
            }
            plan.prepare.push(CommandSpec {
                program: tool(options, root, "javac"),
                args,
                cwd: root.clone(),
            });
        }
    }
    plan.launch.args.extend(options.jvm_args());
    if let Some(port) = debug_port {
        plan.launch.args.push(format!(
            "-agentlib:jdwp=transport=dt_socket,server=y,suspend={},address=127.0.0.1:{port}",
            if suspend { "y" } else { "n" }
        ));
    }
    plan.launch.args.extend([
        "-cp".into(),
        "<resolved-classpath>".into(),
        entry.class.clone(),
    ]);
    plan.launch
        .args
        .extend(options.args.clone().unwrap_or_default());
    Ok(plan)
}

fn matches_group_id(jar_path: &Path, group_id: &str) -> bool {
    let mut current = match jar_path
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
    {
        Some(p) => p,
        None => return false,
    };
    for part in group_id.split('.').rev() {
        match current.file_name().and_then(|s| s.to_str()) {
            Some(name) if name == part => match current.parent() {
                Some(parent) => current = parent,
                None => return false,
            },
            _ => return false,
        }
    }
    true
}

fn find_reactor_replacement(
    jar_path: &Path,
    reactor_modules: &[ReactorModule],
) -> Option<PathBuf> {
    if reactor_modules.is_empty() {
        return None;
    }
    if !jar_path
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("jar"))
    {
        return None;
    }

    let file_name = jar_path.file_name()?.to_str()?;
    let version_dir = jar_path.parent()?.file_name()?.to_str()?;
    let artifact_dir = jar_path.parent()?.parent()?.file_name()?.to_str()?;

    for module in reactor_modules {
        // 1. Directory name must match module.artifact_id exactly (prevents bestwin vs bestwinbase)
        if artifact_dir != module.artifact_id {
            continue;
        }

        // 2. jar filename must match {artifact_id}-{version}.jar or {artifact_id}-{version}-classifier.jar
        let expected_prefix = format!("{}-{version_dir}", module.artifact_id);
        let base_prefix = version_dir
            .strip_suffix("-SNAPSHOT")
            .map(|base| format!("{}-{base}", module.artifact_id));
        let version_matches = file_name.starts_with(&expected_prefix)
            || base_prefix.as_ref().is_some_and(|bp| file_name.starts_with(bp));

        if !version_matches {
            continue;
        }

        // 3. Optional groupId match if available
        if let Some(group_id) = &module.group_id {
            if !matches_group_id(jar_path, group_id) {
                continue;
            }
        }

        // 4. Upstream target/classes must exist
        if module.classes_dir.is_dir() {
            return Some(module.classes_dir.clone());
        }
    }

    None
}

pub fn resolve_classpath(plan: &mut Plan) -> Result<()> {
    if plan.classes.is_empty() {
        return Ok(());
    }
    let mut paths = plan.classes.clone();
    if let Some(file) = &plan.classpath_file {
        let content = std::fs::read_to_string(file)
            .with_context(|| format!("Missing generated classpath: {}", file.display()))?;
        for p in std::env::split_paths(content.trim()).filter(|p| !p.as_os_str().is_empty()) {
            if let Some(replacement) = find_reactor_replacement(&p, &plan.reactor_modules) {
                paths.push(replacement);
            } else {
                paths.push(p);
            }
        }
    }
    let mut seen = HashSet::new();
    paths.retain(|p| seen.insert(p.clone()));

    let cp = std::env::join_paths(paths)?.to_string_lossy().into_owned();
    if let Some(arg) = plan
        .launch
        .args
        .iter_mut()
        .find(|a| a.as_str() == "<resolved-classpath>")
    {
        *arg = cp;
    }
    Ok(())
}

pub fn command(spec: &CommandSpec, env: &BTreeMap<String, String>) -> std::process::Command {
    let mut c = std::process::Command::new(&spec.program);
    c.args(&spec.args).current_dir(&spec.cwd).envs(env);
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_matches_group_id() {
        let p1 = Path::new("/Users/river/tools/mvn_repo/com/bestwin/uis/bestwinbase/V0.6.1/bestwinbase-V0.6.1.jar");
        assert!(matches_group_id(p1, "com.bestwin.uis"));
        assert!(!matches_group_id(p1, "com.other.uis"));
        assert!(!matches_group_id(p1, "org.bestwin.uis"));

        let p2 = Path::new("/repo/commons-io/commons-io/2.11.0/commons-io-2.11.0.jar");
        assert!(matches_group_id(p2, "commons-io"));
        assert!(!matches_group_id(p2, "org.apache.commons"));
    }

    #[test]
    fn test_find_reactor_replacement_and_collision_safety() {
        let tmp = tempfile::tempdir().unwrap();
        let base_classes = tmp.path().join("bestwinbase/target/classes");
        fs::create_dir_all(&base_classes).unwrap();

        let modules = vec![ReactorModule {
            artifact_id: "bestwinbase".into(),
            group_id: Some("com.bestwin.uis".into()),
            classes_dir: base_classes.clone(),
        }];

        // Exact match
        let target_jar = Path::new("/repo/com/bestwin/uis/bestwinbase/V0.6.1/bestwinbase-V0.6.1.jar");
        assert_eq!(
            find_reactor_replacement(target_jar, &modules),
            Some(base_classes.clone())
        );

        // Prefix collision: bestwin must NOT match bestwinbase
        let prefix_jar = Path::new("/repo/com/bestwin/uis/bestwin/V0.6.1/bestwin-V0.6.1.jar");
        assert_eq!(find_reactor_replacement(prefix_jar, &modules), None);

        // GroupId mismatch must NOT match
        let other_group_jar = Path::new("/repo/org/other/uis/bestwinbase/V0.6.1/bestwinbase-V0.6.1.jar");
        assert_eq!(find_reactor_replacement(other_group_jar, &modules), None);

        // Target classes does not exist
        let missing_classes = tmp.path().join("missing/target/classes");
        let modules_missing = vec![ReactorModule {
            artifact_id: "bestwinbase".into(),
            group_id: Some("com.bestwin.uis".into()),
            classes_dir: missing_classes,
        }];
        assert_eq!(find_reactor_replacement(target_jar, &modules_missing), None);
    }

    #[test]
    fn test_resolve_classpath_replaces_reactor_dependency_jar_with_target_classes() {
        let tmp = tempfile::tempdir().unwrap();
        let entry_classes = tmp.path().join("business/target/classes");
        let upstream_classes = tmp.path().join("bestwinbase/target/classes");
        fs::create_dir_all(&entry_classes).unwrap();
        fs::create_dir_all(&upstream_classes).unwrap();

        let fake_repo_jar = tmp.path().join("repo/com/bestwin/uis/bestwinbase/V0.6.1/bestwinbase-V0.6.1.jar");
        let third_party_jar = tmp.path().join("repo/org/springframework/boot/spring-boot/2.7.0/spring-boot-2.7.0.jar");
        fs::create_dir_all(fake_repo_jar.parent().unwrap()).unwrap();
        fs::create_dir_all(third_party_jar.parent().unwrap()).unwrap();
        fs::write(&fake_repo_jar, b"fake old jar").unwrap();
        fs::write(&third_party_jar, b"spring jar").unwrap();

        let cp_file = tmp.path().join("test.classpath");
        let cp_content = format!(
            "{}:{}",
            fake_repo_jar.display(),
            third_party_jar.display()
        );
        fs::write(&cp_file, cp_content).unwrap();

        let mut plan = Plan {
            entry: "business::App".into(),
            prepare: vec![],
            launch: CommandSpec {
                program: "java".into(),
                args: vec!["-cp".into(), "<resolved-classpath>".into(), "demo.App".into()],
                cwd: tmp.path().to_path_buf(),
            },
            classpath_file: Some(cp_file),
            classes: vec![entry_classes.clone()],
            notes: vec![],
            environment_keys: vec![],
            reactor_modules: vec![ReactorModule {
                artifact_id: "bestwinbase".into(),
                group_id: Some("com.bestwin.uis".into()),
                classes_dir: upstream_classes.clone(),
            }],
        };

        resolve_classpath(&mut plan).unwrap();

        let resolved_cp = &plan.launch.args[1];
        let items: Vec<PathBuf> = std::env::split_paths(resolved_cp).collect();

        // 1. Entry classes is at front
        assert_eq!(items[0], entry_classes);
        // 2. Upstream reactor dependency is replaced by target/classes
        assert!(items.contains(&upstream_classes));
        // 3. Third-party jar is preserved
        assert!(items.contains(&third_party_jar));
        // 4. Old repo jar is completely eliminated (not just prepended!)
        assert!(!items.contains(&fake_repo_jar));
    }

    #[test]
    fn test_resolve_classpath_preserves_jar_when_build_false() {
        let tmp = tempfile::tempdir().unwrap();
        let entry_classes = tmp.path().join("business/target/classes");
        let upstream_classes = tmp.path().join("bestwinbase/target/classes");
        fs::create_dir_all(&entry_classes).unwrap();
        fs::create_dir_all(&upstream_classes).unwrap();

        let fake_repo_jar = tmp.path().join("repo/com/bestwin/uis/bestwinbase/V0.6.1/bestwinbase-V0.6.1.jar");
        fs::create_dir_all(fake_repo_jar.parent().unwrap()).unwrap();
        fs::write(&fake_repo_jar, b"fake jar").unwrap();

        let cp_file = tmp.path().join("test.classpath");
        fs::write(&cp_file, fake_repo_jar.to_string_lossy().as_bytes()).unwrap();

        // When options.build == false, reactor_modules is empty
        let mut plan = Plan {
            entry: "business::App".into(),
            prepare: vec![],
            launch: CommandSpec {
                program: "java".into(),
                args: vec!["-cp".into(), "<resolved-classpath>".into(), "demo.App".into()],
                cwd: tmp.path().to_path_buf(),
            },
            classpath_file: Some(cp_file),
            classes: vec![entry_classes.clone()],
            notes: vec![],
            environment_keys: vec![],
            reactor_modules: vec![], // empty when build is false
        };

        resolve_classpath(&mut plan).unwrap();

        let resolved_cp = &plan.launch.args[1];
        let items: Vec<PathBuf> = std::env::split_paths(resolved_cp).collect();

        // Repo jar is preserved, not replaced by target/classes
        assert!(items.contains(&fake_repo_jar));
        assert!(!items.contains(&upstream_classes));
    }
}

