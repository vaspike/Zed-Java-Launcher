# Zed Java Launcher

A macOS-first Java launch manager that brings the useful parts of the existing VS Code Java Launcher workflow to Zed without depending on VS Code APIs.

It is currently a native CLI that generates Zed `tasks.json` / `debug.json` and owns the processes it starts. Zed's Java extension still provides language support and native Java debugging.

## What works in v0.1

- Maven reactor scanning with standard `src/main/java` and `src/test/java` layouts.
- Tree-sitter Java parsing for:
  - Spring Boot applications (`@SpringBootApplication` + `main`).
  - ordinary `main` classes.
  - JUnit-style test classes/methods.
- Migration from existing `.vscode/launch.json` and `.vscode/aggregated-launch.json`.
- Persistent per-entry options in `.java-launcher/config.json`.
- Generation/merge of `.zed/tasks.json` and `.zed/debug.json`.
- Single application run/stop/restart through a supervisor process.
- Group up/down with ordered launch requests and delays.
- Native Zed Java debug configs for single Spring Boot applications.
- Literal `.env` loading for CLI-managed processes when configured.
- Automatic Zed lifecycle binding: services started inside Zed automatically terminate when Zed quits (via macOS kernel `kqueue`), configurable via `"terminate_on_zed_quit": true/false` in `.java-launcher/config.json`.

## Important limits

- No custom Zed side bar yet; interaction is through Zed Tasks/Debugger and this CLI.
- macOS/Linux only at this stage.
- Gradle is rejected in v0.1 instead of treated as plain Java.
- Custom Maven source directories are warned but not evaluated.
- Group launch acknowledges supervisor startup, not Spring readiness.
- Test debugging is not implemented.
- CLI-managed processes and Zed-native debug sessions are separate lifecycles.

## TUI

```bash
java-launcher --project /path/to/project ui
```

In Zed, run the generated task `[Java Launcher] Open`. It opens the same TUI inside Zed's terminal.

Keys:

- `r` / `Enter` run selected application or start group
- `s` stop selected application or group
- `R` restart selected application
- `d` run application with JDWP debug port (5005)
- `Space` toggle start/stop
- `l` open log viewer with scroll and refresh
- `o` open log file directly in Zed editor (works in list view, log viewer, and CLI `logs -o`)
- `m` toggle fullscreen in log viewer
- `f` toggle live follow / pause in log viewer
- `p` set/clear Spring profile with pre-filled editor
- `g` / `G` group up / group down (or `Home` / `End` to jump)
- `/` enter fuzzy search mode (`Esc`/`Enter` to confirm, results ranked by score)
- `Tab` cycle entry filter (Spring Boot only / Apps / All)
- `F5` reload Maven reactor and config
- `?` show help popup
- `q` quit

By default `sync-zed` generates the Open task, group tasks, process tasks, and debug configs. Per-entry run/stop/restart tasks are only generated with `--include-main` or `--include-tests`.

## Commands

```bash
java-launcher --project /path/to/project doctor
java-launcher --project /path/to/project scan --spring-only
java-launcher --project /path/to/project init --from-vscode --write
java-launcher --project /path/to/project sync-zed --write --binary /absolute/path/to/java-launcher
java-launcher --project /path/to/project plan gateway
java-launcher --project /path/to/project start gateway
java-launcher --project /path/to/project ps
java-launcher --project /path/to/project logs gateway -f
java-launcher --project /path/to/project logs gateway -o   # Open in Zed
java-launcher --project /path/to/project stop gateway
java-launcher --project /path/to/project group up uis
java-launcher --project /path/to/project group down uis
java-launcher --project /path/to/project profile gateway local
```

`run` is foreground. `start` is background supervisor mode. `prepare` is used by generated Zed debug configs.

## Zed usage

After `sync-zed --write`, open the project in Zed:

- Use `task: spawn` and search `[Java Launcher]` to run, stop, restart, or manage groups.
- Use the debugger UI to choose `[Java Launcher] Debug ...` for native Zed Java debugging.
- Use `[Java Launcher] Refresh configurations` after changing launcher config.

The generated files are merged safely:

- Custom non-launcher tasks/debug configs are preserved.
- Manually edited launcher entries are preserved and no longer auto-managed until moved into `.java-launcher/config.json`.
- Backups are kept in `.zed/.java-launcher-backups/`.

## UIS notes

For `/Users/river/IdeaProjects/uis`, migration found 18 Maven modules and 11 current Spring Boot applications in the reactor. The old VS Code aggregate group `uis` was imported with 10 applications. Old launch entries for modules not in the current Maven reactor (`accounting`, `graph`, `reporting`, `shipping`) were skipped.

No services are started by `init` or `sync-zed`.
