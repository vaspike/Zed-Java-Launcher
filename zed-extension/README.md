# Java Launcher for Zed

A fast, lightweight Java & Spring Boot microservice orchestrator and debugger for the **Zed** editor.

---

## Features

- **Reactor Auto-Discovery**: Automatically parses multi-module Maven projects to find Spring Boot entrypoints, main classes, and target directories.
- **Port Conflict Prevention**: Dynamically assigns isolated JDWP debug ports, enabling seamless concurrent debugging of multiple microservices.
- **Microservice Group Orchestration**: Define service clusters in `.java-launcher/config.json` and launch, restart, or debug entire groups in a single action.
- **Run vs Debug Workflows**:
  - **Debug Mode**: Full DAP debugger attachment with breakpoints, variable inspection, and thread management.
  - **Run Mode**: Instant native JVM execution via Zed tasks with zero debugger handshake and minimal RAM overhead.
- **Zero-Configuration Setup**: Automatically fetches and maintains the native launcher binary matching your platform (macOS Apple Silicon/Intel, Linux x86/ARM64, Windows) directly from official GitHub releases.

---

## Quick Start

### 1. Initialize Your Project
1. Open any Maven Java or Spring Boot project in Zed.
2. Press `Cmd+Shift+P` (macOS) or `Ctrl+Shift+P` (Linux/Windows) -> select **`task: spawn`**.
3. Choose **`[Java Launcher] Initialize Project`**.
   * *This scans your project and generates `.java-launcher/config.json`, `.zed/tasks.json`, and `.zed/launch.json`.*

### 2. Launch Services

#### Debug Mode (with Breakpoints)
1. Open Zed's **Debug** panel.
2. Select any generated debug target:
   - `JL-<ServiceName>`: Debug a specific microservice.
   - `JL-Group-<GroupName>`: Debug an entire microservice group concurrently.
3. Click the **Play (▶️)** button.

#### Run Mode (Fast Native Execution)
1. Press `Cmd+Shift+P` (or `Ctrl+Shift+P`) -> select **`task: spawn`**.
2. Select your target task:
   - `[JL] <ServiceName>: Run`: Launch the service directly without a debugger.
   - `[Java Launcher] Group up <GroupName>`: Launch all services in a group.
   - `[Java Launcher] Group restart <GroupName>`: Recompile and restart all group services.
   - `[Java Launcher] Group down <GroupName>`: Gracefully terminate all running group services.

---

## Configuration (`.java-launcher/config.json`)

The generated configuration file allows fine-grained control over your project:

```json
{
  "active_env": "dev",
  "groups": {
    "core-services": [
      "gateway-service",
      "auth-service",
      "order-service"
    ]
  },
  "services": {
    "order-service": {
      "path": "services/order-service",
      "entry": "com.example.order.OrderApplication",
      "debug_port": 5006,
      "jvm_args": [
        "-Xmx512m",
        "-Dspring.profiles.active=dev"
      ]
    }
  }
}
```

Whenever project modules change, run **`[Java Launcher] Refresh configurations`** from the task menu to re-index.

---

## License

MIT License. See [LICENSE](LICENSE) for details.
