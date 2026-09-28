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
├── install.sh                         # 自动化一键安装脚本
├── uninstall.sh                       # 卸载脚本
└── README.md                          # 本说明文档
```

---

## 安装方法

### 方法一：通过 Zed 编辑器界面手动安装（GUI 方式）

1. 打开 **Zed 编辑器**；
2. 按快捷键 `Cmd+Shift+P` 打开命令面板；
3. 输入并回车执行：**`zed: install dev extension`**；
4. 在弹出的文件选择器中，**直接选中当前的 `zed-java-launcher` 文件夹**；
5. Zed 将立即完成识别与加载（在 `zed: extensions` 列表中会显示 `Java Launcher (Overridden by dev extension)`）；
6. 接着安装命令行工具：
   ```bash
   cp bin/java-launcher ~/.local/bin/
   chmod +x ~/.local/bin/java-launcher
   ```

---

### 方法二：运行一键安装脚本（推荐，1 秒完成）

在终端中进入本目录并执行：

```bash
chmod +x install.sh
./install.sh
```

脚本将自动完成：
1. 拷贝核心 CLI 程序至 `~/.local/bin/java-launcher`；
2. 拷贝标准扩展文件至 Zed 扩展目录 `~/Library/Application Support/Zed/extensions/installed/zed-java-launcher/`。

---

### 方法三：纯文件拷贝（零脚本依赖）

```bash
# 1. 安装 CLI 程序
mkdir -p ~/.local/bin
cp bin/java-launcher ~/.local/bin/
chmod +x ~/.local/bin/java-launcher

# 2. 安装 Zed 扩展
mkdir -p "$HOME/Library/Application Support/Zed/extensions/installed/zed-java-launcher"
cp extension.toml extension.wasm "$HOME/Library/Application Support/Zed/extensions/installed/zed-java-launcher/"
```

---

## 在 Java 项目中启用

在您的 Maven / Spring Boot 项目根目录中运行一次同步命令（例如 `/Users/river/IdeaProjects/uis`）：

```bash
# 自动扫描 Maven 反应堆模块，并在项目的 .zed/ 目录下生成 tasks.json 与 debug.json
java-launcher sync-zed --write
```

如果您的项目之前有 VS Code 配置（`.vscode/launch.json`），还可一键无损导入激活配置与分组：
```bash
java-launcher init --from-vscode --write
java-launcher sync-zed --write
```

---

## 在 Zed 中使用

1. 打开项目，按 `Cmd+Shift+P`；
2. 输入 **`task: spawn`** 并回车；
3. 选择 **`[Java Launcher] Open`** 即可唤起全部 TUI 启动面板、微服务聚合管理与全屏实时日志：
   - `r` / `Enter`：启动选中的 Spring Boot 微服务
   - `s`：停止服务
   - `R`：重启服务
   - `d`：以调试模式（5005 端口）启动
   - `l`：打开实时日志弹窗（`m` 全屏，`o` 在 Zed 中打开日志，`f` 实时滚动跟踪）
   - `o`：直接在 Zed 编辑器中打开当前服务的完整日志文件
   - `p`：交互式编辑 Spring Profile（如 `dev`、`test`、`local`）
   - `/`：按名称实时模糊搜索应用
   - `Space`：一键切换运行状态
4. 点击 Zed 界面底部的 Debugger 面板，可直接使用原生 Java DAP 断点调试单个服务。
