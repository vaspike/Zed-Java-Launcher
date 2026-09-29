# Java Launcher for Zed

[English](#english) | [简体中文](#简体中文)

---

<a name="english"></a>

## English

A fast, no-nonsense Java & Spring Boot microservice orchestrator and debugger for the **Zed** editor.

### Why do I need this?

If you develop multi-module Java / Spring Boot applications in Zed, you've probably hit these roadblocks:
1. **Starting microservices is painful**: You have to open multiple terminals, remember complex Maven/Java commands, or configure launch tasks by hand.
2. **Debugging multiple services crashes**: Zed's default Java debugger binds to JDWP port `5005`. Starting a second service fails immediately with `Address already in use`.
3. **No lightweight "Run" mode**: Zed only gives you DAP debug configs. When you just want your service up and running without debugger overhead, there was no good option.
4. **Writing classpaths by hand is tedious**: Finding full class names like `com.example.warehouse.WarehouseApplication` and configuring working directories is error-prone.

**Java Launcher solves all of this.** It automatically scans your Maven reactor, sets up isolated debug ports, gives you one-click group launches, and provides intelligent auto-completion inside Zed.

---

### Quick Start (30 Seconds)

1. **Install the extension**: Search for `Java Launcher` in Zed's Extensions menu (or install via offline package).
2. **Initialize your project**:
   - Open your Java / Spring Boot project in Zed.
   - Press `Cmd+Shift+P` (or `Ctrl+Shift+P` on Linux) -> select `task: spawn` -> click **`[Java Launcher] Initialize Project`** *(or run `java-launcher init --write` in terminal)*.
   - The plugin automatically scans your project and generates all Zed debug and task configurations.
3. **Run or Debug**:
   - **To Debug**: Click Zed's Debug panel (bottom/left) -> pick `JL-<ServiceName>` or `JL-Group-<GroupName>` -> click ▶️.
   - **To Run (fast, no debugger)**: Press `Cmd+Shift+P` -> `task: spawn` -> pick `[JL] <ServiceName>: Run` or `[Java Launcher] Group up <GroupName>`.
   - **To Refresh**: Whenever you add new services or modules, just click **`[Java Launcher] Refresh configurations`**.

---

### Run vs Debug: Which should I use?

| Feature | **Run Mode** | **Debug Mode** |
| :--- | :--- | :--- |
| **How it starts** | Starts a native JVM process directly in Zed's terminal. | Starts JVM with JDWP and attaches Zed via DAP. |
| **Debug port** | **Zero port usage**. Does not use or touch port 5005. | **Auto-assigned isolated ports**. No conflicts. |
| **Startup speed** | **Instant**. No debugger handshake; lowest RAM usage. | Normal boot time with breakpoint indexing. |
| **Where to trigger** | Press `Cmd+Shift+P` -> `task: spawn` -> pick `[JL] <Service>: Run` or `[Java Launcher] Group up <Group>` | Zed Debug Panel -> select `JL-<Service>` or `JL-Group-<Group>` -> click ▶️ |
| **Best used for** | Running dependencies, daily local testing, running apps you don't need breakpoints in. | Stepping through code, setting breakpoints, viewing variables and stack traces. |

---

### How to Create & Manage Groups

A **Group** allows you to launch, restart, or debug a cluster of microservices with a single click.

#### Step 1: Open `.java-launcher/config.json`
*(If it doesn't exist yet, run `[Java Launcher] Initialize Project` in Zed to create it).*

#### Step 2: Add your group under `"groups"`
```jsonc
{
  "$schema": "./config.schema.json",
  "version": 1,
  "groups": {
    "my-cluster": [
      {
        "entry": "gateway::com.example.GatewayApplication",
        "delay_ms": 0,    // start immediately
        "enabled": true
      },
      {
        "entry": "order-service::com.example.OrderApplication",
        "delay_ms": 2000, // wait 2 seconds after gateway before launching
        "enabled": true
      },
      {
        "entry": "payment-service::com.example.PaymentApplication",
        "delay_ms": 1000,
        "enabled": false  // set to false to temporarily skip this service
      }
    ]
  }
}
```

> 💡 **Auto-Completion Tip**: When you type `"entry": "..."`, Zed will automatically pop up a dropdown list containing **every valid Java service in your project**. Just hit Enter to insert it. You never have to manually look up class paths.

#### Step 3: Refresh and run
Save the file and run `[Java Launcher] Refresh configurations`. Zed will immediately register:
- **In Debug Panel**: `JL-Group-my-cluster` (debugs all services concurrently without port conflicts).
- **In Task List**:
  - `[Java Launcher] Group up my-cluster` (starts all services in background)
  - `[Java Launcher] Group restart my-cluster` (gracefully stops all in reverse order, then starts them up)
  - `[Java Launcher] Group down my-cluster` (stops all services in the group)

---

### What Files Does Java Launcher Generate?

Java Launcher is completely transparent. Here is every file it creates in your project:

| File / Folder | Managed by | Purpose |
| :--- | :---: | :--- |
| **`.java-launcher/config.json`** | You & Tool | **Your main config file**. Stores groups, per-service options (Spring profiles, JVM args), and defaults. Comments (JSONC) are supported and preserved. Never wiped out. |
| **`.java-launcher/config.schema.json`** | Tool | **Auto-completion schema**. Updated on every project scan so Zed knows the exact list of runnable services in your project for dropdown suggestions. |
| **`.zed/tasks.json`** | Tool (Merged) | **Zed task list**. Registers group commands, log cleaning, and individual `[JL] <Service>: Run / Restart / Stop` tasks. Your own custom tasks here are preserved. |
| **`.zed/debug.json`** | Tool (Merged) | **Zed debug configs**. Registers `JL-<Service>` and `JL-Group-<Group>` DAP configs. Your own remote attach configs are preserved. |
| **`.zed/.java-launcher-backups/`** | Tool | **Safe backups**. Timestamped backups taken before modifying `tasks.json` or `debug.json`. |
| **`~/.local/state/java-launcher/<hash>/*.log`** | Runtime | **Service log files**. Output from background services. Safe to clear anytime with `[Java Launcher] Clean logs`. |

---

### Daily Tasks Cheat Sheet

Press `Cmd+Shift+P` -> type `task: spawn`:

- **`[Java Launcher] Initialize Project`**: Initial bootstrap task (global). Scans a fresh project and creates all Zed tasks and debug configs.
- **`[Java Launcher] Refresh configurations`**: Scans the project, updates tasks and debug configs (Clean Mode, recommended).
- **`[Java Launcher] Refresh configurations (include main)`**: Full sync mode. Also generates individual Run / Stop / Restart tasks for every service in your project.
- **`[Java Launcher] Group up <group>`**: Starts all services in the group in order.
- **`[Java Launcher] Group restart <group>`**: Gracefully restarts the group.
- **`[Java Launcher] Group down <group>`**: Stops all services in the group and releases ports.
- **`[Java Launcher] Clean logs`**: Truncates all service logs to 0 bytes and frees disk space (safe for running services).
- **`[Java Launcher] Running processes`**: Lists all active background services managed by Java Launcher.
- **`[Java Launcher] Stop all managed processes`**: Stops all running background services in this project.
- **`[JL] <Service>: Run`**: Runs this single service in the foreground terminal (press `Ctrl+C` to stop).
- **`[JL] <Service>: Restart`**: Restarts this single background service.
- **`[JL] <Service>: Stop`**: Stops this single background service.

> 💡 **Clean Mode vs Full Mode (Prevent Task Palette Clutter)**:
> In microservice projects with dozens of services, generating `Run`, `Restart`, and `Stop` for every single app can spam your Zed task palette with 60+ entries.
> - **`Refresh configurations` (Clean Mode, recommended)**: Keeps your task palette tidy by only generating Group and admin tasks (individual services can always be debugged from the Debug panel).
> - **`Refresh configurations (include main)` (Full Mode)**: Generates individual `[JL] <Service>: Run / Restart / Stop` tasks for every service in your project.
> - **Easily reversible**: Switching back to clean mode is as simple as clicking the default `Refresh configurations` task again anytime.

---

### Migrating from VS Code

If you have an existing project configured with the VS Code Java Launcher extension (`.vscode/launch.json` and `.vscode/aggregated-launch.json`), you can import everything in one command:
```bash
java-launcher init --from-vscode --write
```
This automatically imports your groups, Spring profiles, and arguments into `.java-launcher/config.json`.

---

### Requirements

- **OS**: macOS (Apple Silicon & Intel) or Linux (Ubuntu, Debian, Fedora, Arch, etc.), or Windows WSL2.
- **JDK**: Java 17+ recommended for running Zed's JDTLS language server; your projects can be Java 8, 11, 17, or 21.
- **Zed Extension**: Official `java` extension installed in Zed.

---

<br/>
<br/>

---

<a name="简体中文"></a>

## 简体中文

面向 **Zed 编辑器** 的多模块 Java 与 Spring Boot 微服务一站式启动器、生命周期编排与 DAP 断点调试管理器。

### 为什么需要这个插件？

在 Zed 里开发 Java 多模块或微服务项目时，原生体验有几个明显的痛点：
1. **启动多个服务极其繁琐**：通常要在多个终端 Tab 里分别敲命令，或者手动编写复杂的启动任务。
2. **多服务同时调试必冲突**：Zed 默认的 Java 调试器会锁死在 JDWP `5005` 端口，只要启动第二个微服务的调试，就会直接报端口冲突错误。
3. **缺少轻量级的直接运行模式（Run）**：Zed 原生只有 DAP 调试配置。如果日常只是想把服务直接跑起来（不需要断点调试），每次走调试器不仅启动慢，而且非常吃内存。
4. **手写类路径太容易出错**：手动在 JSON 里输入 `com.example.warehouse.WarehouseApplication` 和模块名，不仅麻烦还容易打错。

**Java Launcher 就是为了解决这些问题而生的。** 它会自动扫描工程的 Maven 反应堆，自动处理多服务调试端口隔离，提供一键组启动与组重启，并在 Zed 中提供微服务下拉自动补全。

---

### 30 秒极速上手

1. **安装插件**：在 Zed 扩展面板中搜索 `Java Launcher` 并安装（或使用离线安装包）；
2. **一键初始化项目**：
   - 在 Zed 中打开任意 Java / Spring Boot 项目；
   - 按快捷键 `Cmd+Shift+P`（Linux 为 `Ctrl+Shift+P`）$\rightarrow$ 搜索 `task: spawn` $\rightarrow$ 点击 **`[Java Launcher] Initialize Project`**；*(也可以在项目终端中执行 `java-launcher init --write`)*；
   - 插件会自动扫描所有 Maven 模块，在项目下生成 `.zed` 与 `.java-launcher` 配置；
3. **开始运行与调试**：
   - **调试（Debug）**：点击 Zed 底部/左侧的 Debug 面板 $\rightarrow$ 选择 `JL-<服务名>` 或 `JL-Group-<组名>` $\rightarrow$ 点击 ▶️ 即可进入断点调试；
   - **运行（Run，极速无调试器）**：按 `Cmd+Shift+P` $\rightarrow$ `task: spawn` $\rightarrow$ 选择 **`[JL] <服务名>: Run`** 或 **`[Java Launcher] Group up <组名>`**；
   - **后续刷新**：工程新增模块或微服务后，随时点击 **`[Java Launcher] Refresh configurations`** 即可同步最新配置。

---

### Run 启动 vs Debug 调试：该选哪个？

| 比较维度 | **Run 启动（极速运行）** | **Debug 调试（DAP 断点）** |
| :--- | :--- | :--- |
| **底层通道** | 原生 JVM 进程直接拉起，**不挂 JDWP 端口，不走调试协议**。 | 挂载 JDWP 端口，**通过 DAP 协议双向桥接 Zed 与 JDTLS**。 |
| **端口占用** | **完全零端口占用**。绝对不会占用 5005 端口。 | **动态多端口隔离**。多服务同时调试互不干扰，零端口冲突。 |
| **启动速度** | **秒级极速拉起**。无调试器握手开销，内存占用最低。 | 包含符号索引与断点初始化，正常编译启动速度。 |
| **操作入口** | 按 `Cmd+Shift+P` $\rightarrow$ `task: spawn` 选择 `[JL] <服务名>: Run` 或 `Group up <组名>` | 在 Zed Debug 面板选择 `JL-<服务名>` 或 `JL-Group-<组名>` 点击 ▶️ |
| **适合场景** | 本地业务联调、组装依赖集群、启动不需要下断点的服务。 | 单步调试排查 Bug、命中代码断点、查看 Call Stack 与变量值。 |

---

### 如何创建与管理服务组（Group）

通过服务组（Group），你可以一键拉起、重启或停止一整套微服务集群，并能精确控制每个服务的先后启动延迟。

#### 第一步：打开 `.java-launcher/config.json`
*(若项目中还没有该文件，先在 Zed 任务中运行一次 `[Java Launcher] Initialize Project` 即可自动生成)*。

#### 第二步：在 `"groups"` 节点下添加你的组
```jsonc
{
  "$schema": "./config.schema.json",
  "version": 1,
  "groups": {
    "my-cluster": [
      {
        "entry": "gateway::com.example.GatewayApplication",
        "delay_ms": 0,    // 立即启动
        "enabled": true
      },
      {
        "entry": "order-service::com.example.OrderApplication",
        "delay_ms": 2000, // 在网关拉起 2 秒后再启动，确保依赖就绪
        "enabled": true
      },
      {
        "entry": "payment-service::com.example.PaymentApplication",
        "delay_ms": 1000,
        "enabled": false  // 设为 false 可在组内临时跳过该服务
      }
    ]
  }
}
```

> 💡 **自动补全小技巧**：在输入 `"entry": "` 时，Zed 会**自动弹出下拉框，列出当前工程里所有真实存在的微服务**。按回车直接填入，完全不需要去手动翻包名路径。

#### 第三步：刷新配置并使用
保存文件后，在 Zed 中运行任务 **`[Java Launcher] Refresh configurations`**，Zed 会立刻自动注册：
- **Debug 调试面板**：`JL-Group-my-cluster`（一键同时调试组内所有服务，断点互不冲突）；
- **Task 任务列表**：
  - `[Java Launcher] Group up my-cluster`（后台按顺序启动整组服务）
  - `[Java Launcher] Group restart my-cluster`（倒序平滑关停，然后重新按顺序拉起）
  - `[Java Launcher] Group down my-cluster`（停止整组服务并释放端口）

---

### 插件会在我项目里生成哪些文件？

Java Launcher 遵循极简和透明原则，以下是插件涉及的所有文件与作用说明：

| 文件 / 目录 | 维护方 | 作用与说明 |
| :--- | :---: | :--- |
| **`.java-launcher/config.json`** | 开发者 | **主配置文件**。保存微服务组（`groups`）、各服务参数覆盖（`entries` 如 Spring Profile、JVM 参数）和全局默认配置。支持 JSONC 注释，永远不会被插件刷新覆盖。 |
| **`.java-launcher/config.schema.json`** | 插件自动 | **自动补全定义文件**。每次扫描工程时更新，将工程中真实存在的微服务 ID 注入为候选列表，让 Zed 在编辑 `config.json` 时提供字段说明与下拉补全。 |
| **`.zed/tasks.json`** | 插件增量合并 | **Zed 任务文件**。注册组启停、清理日志以及各服务的 `[JL] <服务>: Run/Restart/Stop` 任务。您自己写的手动任务会被安全保留。 |
| **`.zed/debug.json`** | 插件增量合并 | **Zed 调试文件**。注册 `JL-<服务>` 与 `JL-Group-<组>` 的 DAP 调试配置。您自己写的手动远程 attach 配置会被安全保留。 |
| **`.zed/.java-launcher-backups/`** | 插件自动 | **安全备份目录**。每次修改 `tasks.json` 或 `debug.json` 前自动保存带时间戳的备份。 |
| **`~/.local/state/java-launcher/<hash>/*.log`** | 运行时生成 | **后台服务运行日志**。记录后台托管进程的标准输出。随时可在 Zed 中运行 `[Java Launcher] Clean logs` 安全清零释放磁盘。 |

---

### 日常高频任务速查表

在 Zed 中按 `Cmd+Shift+P` $\rightarrow$ 输入 `task: spawn`：

- **`[Java Launcher] Initialize Project`**：全局初始引导任务。在新工程中一键扫描并生成全部 Zed 任务与调试配置。
- **`[Java Launcher] Refresh configurations`**：重新扫描工程，刷新任务与调试配置（清爽模式，不生成单服务任务列表）。
- **`[Java Launcher] Refresh configurations (include main)`**：全量刷新模式。会在任务列表里为每个单服务生成对应的 `Run` / `Restart` / `Stop` 任务。
- **`[Java Launcher] Group up <组名>`**：后台按顺序拉起组内全部微服务。
- **`[Java Launcher] Group restart <组名>`**：平滑倒序停止组内服务，重新编译并顺序拉起。
- **`[Java Launcher] Group down <组名>`**：平滑关停组内所有服务并释放端口。
- **`[Java Launcher] Clean logs`**：将所有微服务的日志文件安全截断清空（`0 字节`），快速释放磁盘空间（服务运行中也能安全清理）。
- **`[Java Launcher] Running processes`**：列出当前正在后台运行的受管微服务与 PID。
- **`[Java Launcher] Stop all managed processes`**：一键停止当前项目由 Java Launcher 启动的所有后台服务。
- **`[JL] <服务名>: Run`**：在前台终端运行该单服务（控制台直显日志，按 `Ctrl+C` 直接终止）。
- **`[JL] <服务名>: Restart`**：单独重启该后台服务。
- **`[JL] <服务名>: Stop`**：单独停止该后台服务。

> 💡 **清爽模式 vs 全量模式（告别任务列表刷屏）**：
> 大型工程往往有几十个微服务，如果无脑把每个服务的 `Run`、`Restart`、`Stop` 全写进任务列表，会导致 Zed 快捷键弹窗瞬间被几十上百个任务挤爆。
> - **`Refresh configurations`（清爽模式，推荐）**：只生成组任务与全局运维，保护任务列表不被微服务刷屏（需要单服务调试时，在 Debug 面板直接点即可）。
> - **`Refresh configurations (include main)`（全量模式）**：如果你习惯在 Zed 任务弹窗里键盘搜单个服务，点击此任务会为每个单服务生成对应的 `[JL] <服务名>: Run / Restart / Stop` 任务。
> - **随时一键还原**：觉得任务太多时，只需重新点击一次默认的 `Refresh configurations`，未手动修改过的单服务任务会被自动清空，瞬间恢复清爽！

---

### 从 VS Code 平滑迁移

如果您的项目之前在 VS Code 中使用过 Java Launcher 插件（包含 `.vscode/launch.json` 和 `.vscode/aggregated-launch.json`）：
只需在终端中执行一行命令：
```bash
java-launcher init --from-vscode --write
```
插件会自动将所有旧的分组定义、启动参数和 Spring Profile 无缝迁移至 `.java-launcher/config.json`。

---

### 环境要求

- **操作系统**：macOS (Apple Silicon / Intel)、Linux 各主流发行版（Ubuntu/Debian/Fedora/Arch 等）、或 Windows WSL2；
- **JDK 环境**：本地需安装 JDK（推荐 JDK 17 或 21，兼容编译运行 Java 8/11/17/21 业务工程）；
- **Zed 依赖**：需在 Zed 中安装官方提供的 **`java`** 扩展（用于提供 JDTLS 代码分析支持）。
