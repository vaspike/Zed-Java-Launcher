# Zed Java Launcher 插件标准发行包

面向 **Zed 编辑器** 的多模块 Maven 与 Spring Boot 微服务一站式启动器、生命周期管理器与交互式终端 UI（TUI）。

---

## 插件包标准结构

本目录是严格遵循 **Zed 官方标准扩展格式** 的目录结构，根目录直接包含 `extension.toml` 与预编译好的 WebAssembly 模块：

```text
zed-java-launcher/                     # <--- 在 Zed 中通过 "Install Dev Extension" 选中的正是此目录
├── extension.toml                     # Zed 扩展标准清单（位于根目录）
├── extension.wasm                     # 预编译 Zed WebAssembly 扩展组件
├── Cargo.toml                         # Zed WASM 扩展构建清单
├── src/
│   └── lib.rs                         # Zed WASM 扩展源码
├── bin/
│   └── java-launcher                  # 预编译原生 CLI / TUI 核心二进制 (macOS arm64)
├── install.sh                         # 自动化一键安装脚本（自动配置全局引导任务）
├── uninstall.sh                       # 卸载脚本
└── README.md                          # 本说明文档
```

---

## 安装方法

### 方法一：运行一键安装脚本（最推荐，零手动配置）

在终端中进入本目录并执行：

```bash
chmod +x install.sh
./install.sh
```

脚本将自动完成：
1. 拷贝核心 CLI 程序至 `~/.local/bin/java-launcher`；
2. 拷贝标准扩展文件至 Zed 扩展目录 `~/Library/Application Support/Zed/extensions/installed/zed-java-launcher/`；
3. **自动向 Zed 全局任务库（`~/.config/zed/tasks.json`）注册全局引导任务**。

安装完成后，打开任何项目，直接在 Zed 内按 `Ctrl+T` 就能看到初始化任务，完全不需要打开外部终端！

---

### 方法二：通过 Zed 编辑器界面手动安装（GUI 方式）

1. 打开 **Zed 编辑器**；
2. 按快捷键 `Cmd+Shift+P` 打开命令面板；
3. 输入并回车执行：**`zed: install dev extension`**；
4. 在弹出的文件选择器中，**直接选中当前的 `zed-java-launcher` 文件夹**；
5. Zed 将立即完成识别与加载；
6. 接着安装命令行工具：
   ```bash
   cp bin/java-launcher ~/.local/bin/
   chmod +x ~/.local/bin/java-launcher
   ```
7. 如需在 Zed 内随时初始化新项目，可按 `Cmd+Shift+P` -> `zed: open tasks`，将以下内容追加保存：
   ```json
   [
     {
       "label": "Java Launcher: Initialize Project (.zed)",
       "command": "java-launcher",
       "args": ["sync-zed", "--write"],
       "use_new_terminal": false,
       "allow_concurrent_runs": false
     },
     {
       "label": "Java Launcher: Open Control Panel (TUI)",
       "command": "java-launcher",
       "args": ["ui"],
       "use_new_terminal": true,
       "allow_concurrent_runs": false
     }
   ]
   ```

---

## 在 Zed 中使用（100% 纯界面，告别命令行）

1. **新项目初次使用**：
   在 Zed 中打开任意 Java 项目，按下 `Ctrl+T`（或 `Cmd+Shift+P` -> `task: spawn`），选择 **`Java Launcher: Initialize Project (.zed)`**。Zed 会自动完成微服务扫描并生成项目任务！
2. **日常开发与微服务编排**：
   再次按 `Ctrl+T`，直接选择：
   - **`[Java Launcher] Open`**：唤起交互式控制面板（启停、换 Profile、实时日志、搜服务）
   - **`[Java Launcher] Up uis`**：一键按编排拉起全部微服务
   - **`[Java Launcher] Down uis`**：一键倒序停止整个集群
3. **原生断点调试**：
   点击 Zed 底部 Debugger 面板，选择对应微服务直接点击 ▶️ 开启 Java DAP 断点调试。
