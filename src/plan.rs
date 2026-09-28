use crate::{
    config,
    model::{Entry, LaunchOptions, Project},
    runtime,
};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
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
            args.extend(["install".into(), "-DskipTests".into()]);
            plan.prepare.push(CommandSpec {
                program: program.clone(),
                args,
                cwd: root.clone(),
            });
            plan.notes.push("Maven install builds the selected module and upstream dependencies and writes to the local Maven repository; project build plugins WILL execute".into());
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

pub fn resolve_classpath(plan: &mut Plan) -> Result<()> {
    if plan.classes.is_empty() {
        return Ok(());
    }
    let mut paths = plan.classes.clone();
    if let Some(file) = &plan.classpath_file {
        let content = std::fs::read_to_string(file)
            .with_context(|| format!("Missing generated classpath: {}", file.display()))?;
        paths.extend(std::env::split_paths(content.trim()).filter(|p| !p.as_os_str().is_empty()));
    }
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
