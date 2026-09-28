# Java Launcher Extension for Zed

Java Launcher provides seamless multi-module Maven and Spring Boot application discovery, management, debugging, and terminal UI integration for the Zed editor.

## Features

- **Spring Boot & Application Discovery**: Automatically scans Maven reactor modules and indexes main applications.
- **Interactive TUI**: Real-time process manager, terminal log viewer, Spring profile switcher, and fuzzy search.
- **Zed Native Integration**:
  - `tasks.json`: Provides `[Java Launcher] Open` to launch TUI, plus group launch (`Up`/`Down`) and process management.
  - `debug.json`: Native DAP debug configurations for single microservices.
  - Automated Zed lifecycle binding (services terminate automatically on Zed exit).
- **Log Viewer**: View live tail logs in TUI, fullscreen viewing (`m`), and instant file opening in Zed editor (`o`).

## Getting Started

1. Ensure the `java-launcher` CLI binary is installed in your `$PATH` (e.g. `~/.local/bin/java-launcher` or `~/.cargo/bin/java-launcher`).
2. Run `java-launcher --project /path/to/project sync-zed --write` in your Java workspace root.
3. Open Zed, press `Cmd+Shift+P`, and run `task: spawn` -> `[Java Launcher] Open`.
