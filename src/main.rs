#![cfg(unix)]
mod config;
mod dap;
mod model;
mod plan;
mod runtime;
mod scan;
mod zed;

use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use model::EntryKind;
use std::{path::PathBuf, thread, time::Duration};

#[derive(Parser)]
#[command(
    name = "java-launcher",
    version,
    about = "Java launch management and DAP orchestrator for Zed"
)]
struct Cli {
    #[arg(long, global = true, default_value = ".")]
    project: PathBuf,
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Action>,
}
#[derive(Args)]
struct RunArgs {
    /// Stable ID, full class name, or unambiguous module name. Never selects first match.
    entry: String,
    #[arg(long)]
    skip_build: bool,
    /// Open JDWP on 127.0.0.1 only; then use a matching Zed attach configuration.
    #[arg(long)]
    debug_port: Option<u16>,
    #[arg(long, requires = "debug_port")]
    suspend: bool,
    /// Watch a parent process PID and automatically terminate when it exits.
    #[arg(long)]
    watch_pid: Option<u32>,
}
#[derive(Subcommand)]
enum Action {
    /// Check local executables without changing settings or downloading tools.
    Doctor,
    /// Read-only source/AST and Maven reactor scan.
    Scan {
        #[arg(long)]
        json: bool,
        #[arg(long)]
        spring_only: bool,
        #[arg(long)]
        query: Option<String>,
    },
    /// Preview config. Add --write to persist it (existing customizations win).
    Init {
        #[arg(long)]
        from_vscode: bool,
        #[arg(long)]
        write: bool,
    },
    /// Preview Zed tasks/debug JSON. --write installs them in PROJECT/.zed.
    SyncZed {
        #[arg(long)]
        write: bool,
        /// Write a preview bundle elsewhere, without touching the project.
        #[arg(long, conflicts_with = "write")]
        output_dir: Option<PathBuf>,
        /// Include ordinary main methods. Default is Spring Boot only.
        #[arg(long)]
        include_main: bool,
        #[arg(long)]
        include_tests: bool,
        /// Path to a stable installed java-launcher binary for generated tasks.
        #[arg(long)]
        binary: Option<PathBuf>,
    },
    /// Print exact commands; does not build or start anything.
    Plan(RunArgs),
    /// Build and run in the foreground; Ctrl-C stops the owned process group.
    Run(RunArgs),
    /// Start a background supervisor; acknowledgement does NOT mean service readiness.
    Start(RunArgs),
    /// Build only (used as Zed's debug build task).
    Prepare(RunArgs),
    /// List live processes owned by this tool, not unrelated JVMs/Zed sessions.
    Ps,
    /// Stop one managed entry or all managed entries in this project.
    Stop {
        #[arg(required_unless_present = "all", conflicts_with = "all")]
        entry: Option<String>,
        #[arg(long)]
        all: bool,
    },
    /// Stop and start an entry using current persistent config.
    Restart(RunArgs),
    /// Persist the Spring profile without changing other launch options.
    Profile { entry: String, profile: String },
    /// View or tail real-time logs for a managed entry.
    Logs {
        /// Stable ID, full class name, or unambiguous module name.
        entry: String,
        /// Stream log output in real time (like tail -f).
        #[arg(short, long)]
        follow: bool,
        /// Number of lines to show from the end of the log.
        #[arg(short = 'n', long, default_value_t = 100)]
        lines: usize,
        /// Open the log file directly in Zed editor.
        #[arg(short = 'o', long)]
        zed: bool,
    },
    /// Run Debug Adapter Protocol (DAP) server for Zed integration.
    Dap {
        #[arg(long)]
        name: Option<String>,
    },
    /// Start/stop a named group. Delays sequence launch requests, not health checks.
    Group {
        #[arg(value_enum)]
        action: GroupAction,
        name: String,
        #[arg(long)]
        skip_build: bool,
    },
}
#[derive(Clone, ValueEnum)]
enum GroupAction {
    Up,
    Down,
}
fn print_json(value: &impl serde::Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
fn warn(warnings: &[String]) {
    for w in warnings {
        eprintln!("warning: {w}");
    }
}
fn main() {
    if let Err(error) = execute() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}
fn execute() -> Result<()> {
    let cli = Cli::parse();
    let root = cli
        .project
        .canonicalize()
        .context("Project path does not exist")?;
    let config_path = cli
        .config
        .map(|p| {
            if p.is_absolute() {
                p
            } else {
                std::env::current_dir().unwrap().join(p)
            }
        })
        .unwrap_or_else(|| root.join(".java-launcher/config.json"));
    let command = cli.command.unwrap_or(Action::Dap { name: None });
    if let Action::Dap { name } = command {
        let stdin = std::io::stdin();
        let server = dap::DapServer::new(std::io::stdout(), root, config_path, name);
        return server.run(stdin.lock());
    }
    if matches!(command, Action::Doctor) {
        for (name, args) in [
            ("zed", vec!["--version"]),
            ("java", vec!["-version"]),
            ("javac", vec!["-version"]),
            ("mvn", vec!["-version"]),
        ] {
            match std::process::Command::new(name).args(args).output() {
                Ok(out) => {
                    println!("{name}: {}", out.status);
                    print!(
                        "{}{}",
                        String::from_utf8_lossy(&out.stdout),
                        String::from_utf8_lossy(&out.stderr)
                    );
                }
                Err(e) => println!("{name}: unavailable ({e})"),
            }
        }
        println!(
            "Project: {}\nConfig: {}\nState: {}",
            root.display(),
            config_path.display(),
            runtime::state_dir(&root)?.display()
        );
        println!("Requires Zed Java extension. JDTLS needs its own compatible JDK (usually 21+); project JDK is independently configured via defaults.java_home.");
        return Ok(());
    }
    if matches!(command, Action::Ps) {
        return print_json(&runtime::statuses(&root)?);
    }
    if let Action::Logs {
        entry,
        follow,
        lines,
        zed,
    } = &command
    {
        let id = if runtime::request(&root, entry, "status").is_ok() {
            entry.clone()
        } else {
            scan::scan(&root)?.entry(entry)?.id.clone()
        };
        let log_path = match runtime::request(&root, &id, "status") {
            Ok(s) => PathBuf::from(s.log),
            Err(_) => runtime::state_dir(&root)?.join(format!("{}.log", runtime::hash(&id))),
        };
        if *zed {
            return runtime::open_in_zed(&log_path);
        }
        if !log_path.exists() {
            bail!(
                "Log file not found for {id}: {}\nHas the service been started at least once?",
                log_path.display()
            );
        }
        let mut cmd = std::process::Command::new("tail");
        cmd.arg("-n").arg(lines.to_string());
        if *follow {
            cmd.arg("-f");
        }
        cmd.arg(&log_path);
        let status = cmd.status().context("Failed to run tail command")?;
        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }
        return Ok(());
    }
    if let Action::Stop { entry, all } = &command {
        if *all {
            for s in runtime::statuses(&root)? {
                runtime::stop(&root, &s.entry)?;
                println!("Stopped {}", s.entry);
            }
        } else {
            let query = entry.as_ref().unwrap();
            let id = if runtime::request(&root, query, "status").is_ok() {
                query.clone()
            } else {
                scan::scan(&root)?.entry(query)?.id.clone()
            };
            runtime::stop(&root, &id)?;
            println!("Stopped {id}");
        }
        return Ok(());
    }
    // Group down must still work after source files or modules have been removed.
    let mut config = config::load(&config_path)?;
    if let Action::Group {
        action: GroupAction::Down,
        name,
        ..
    } = &command
    {
        let items = config
            .groups
            .get(name)
            .with_context(|| format!("Unknown group {name}"))?;
        for item in items.iter().rev() {
            if runtime::request(&root, &item.entry, "status").is_ok() {
                runtime::stop(&root, &item.entry)?;
                println!("Stopped {}", item.entry);
            }
        }
        return Ok(());
    }
    let project = scan::scan(&root)?;
    warn(&project.warnings);
    match command {
        Action::Scan {
            json,
            spring_only,
            query,
        } => {
            if json {
                return print_json(&project);
            }
            println!(
                "{} modules, {} entries",
                project.modules.len(),
                project.entries.len()
            );
            for e in &project.entries {
                if spring_only && e.kind != EntryKind::SpringBoot {
                    continue;
                }
                if query
                    .as_ref()
                    .is_some_and(|q| !e.id.to_lowercase().contains(&q.to_lowercase()))
                {
                    continue;
                }
                println!("{:?}\t{}\t{}:{}", e.kind, e.id, e.file, e.line);
            }
        }
        Action::Init { from_vscode, write } => {
            if from_vscode {
                warn(&config::import_vscode(&project, &mut config)?);
            }
            if write {
                config::atomic_json(&config_path, &config)?;
                println!("Wrote {}", config_path.display());
            } else {
                print_json(&config)?;
            }
        }
        Action::SyncZed {
            write,
            output_dir,
            include_main,
            include_tests,
            binary,
        } => {
            let binary = binary
                .unwrap_or(std::env::current_exe()?)
                .canonicalize()
                .context("Launcher binary not found")?;
            let generated = zed::generate(
                &project,
                &config,
                &config_path,
                &binary,
                include_main,
                include_tests,
            )?;
            if write || output_dir.is_some() {
                let dir = output_dir.unwrap_or_else(|| root.join(".zed"));
                warn(&zed::sync(&dir, generated)?);
                println!("Wrote {}/tasks.json and debug.json", dir.display());
            } else {
                print_json(&generated)?;
            }
        }
        Action::Plan(ref args)
        | Action::Run(ref args)
        | Action::Start(ref args)
        | Action::Prepare(ref args)
        | Action::Restart(ref args) => {
            let entry = project.entry(&args.entry)?;
            let mut options = config.options(&entry.id);
            if args.skip_build {
                options.build = Some(false);
            }
            let plan = plan::build(&project, entry, &options, args.debug_port, args.suspend)?;
            match &command {
                Action::Plan(_) => print_json(&plan)?,
                Action::Start(_) | Action::Restart(_) => {
                    // Validate environment before spawning, without logging its values.
                    config::environment(&root, &options)?;
                    if matches!(command, Action::Restart(_))
                        && runtime::request(&root, &entry.id, "status").is_ok()
                    {
                        runtime::stop(&root, &entry.id)?;
                    }
                    print_json(&runtime::start(
                        &root,
                        &config_path,
                        &entry.id,
                        args.skip_build,
                        args.debug_port,
                        args.suspend,
                        args.watch_pid,
                    )?)?;
                }
                Action::Prepare(_) => runtime::run(&project, entry, &options, plan, None, true, None)?,
                _ => runtime::run(&project, entry, &options, plan, args.debug_port, false, args.watch_pid)?,
            }
        }
        Action::Profile { entry, profile } => {
            let entry = project.entry(&entry)?;
            if entry.kind != EntryKind::SpringBoot {
                bail!("Profile command requires a Spring Boot entry");
            }
            if profile.chars().any(char::is_whitespace) {
                bail!(
                    "Profile must not contain whitespace; comma-separated profiles are supported"
                );
            }
            config
                .entries
                .entry(entry.id.clone())
                .or_default()
                .spring_profile = Some(profile);
            config::atomic_json(&config_path, &config)?;
            println!(
                "Saved {}. Run sync-zed --write to update native debug configurations.",
                config_path.display()
            );
        }
        Action::Group {
            action: GroupAction::Up,
            name,
            skip_build,
        } => {
            let items = config
                .groups
                .get(&name)
                .with_context(|| format!("Unknown group {name}"))?;
            let enabled: Vec<_> = items.iter().filter(|i| i.enabled).collect();
            if enabled.is_empty() {
                bail!("Group has no enabled items");
            }
            let mut ids = std::collections::BTreeSet::new();
            // Validate EVERY member before starting any process.
            for item in &enabled {
                let entry = project.entry(&item.entry)?;
                if entry.kind.is_test() {
                    bail!(
                        "Group member is a test, not a long-running application: {}",
                        entry.id
                    );
                }
                if !ids.insert(&entry.id) {
                    bail!("Duplicate group member {}", entry.id);
                }
                if runtime::request(&root, &entry.id, "status").is_ok() {
                    bail!("Group member already running: {}", entry.id);
                }
                let mut o = config.options(&entry.id);
                if skip_build {
                    o.build = Some(false);
                }
                plan::build(&project, entry, &o, None, false)?;
                config::environment(&root, &o)?;
            }
            let mut started = vec![];
            for item in enabled {
                thread::sleep(Duration::from_millis(item.delay_ms));
                match runtime::start(&root, &config_path, &item.entry, skip_build, None, false, None) {
                    Ok(status) => {
                        started.push(item.entry.clone());
                        print_json(&status)?;
                    }
                    Err(e) => {
                        for id in started.iter().rev() {
                            let _ = runtime::stop(&root, id);
                        }
                        return Err(e);
                    }
                }
            }
            println!("Launch requests acknowledged. Check `ps` and logs for build/runtime failures; this is not a readiness check.");
        }
        _ => unreachable!(),
    }
    Ok(())
}
