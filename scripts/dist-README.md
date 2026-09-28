# Zed Java Launcher 插件发行包

面向 **Zed 编辑器** 的多模块 Maven 与 Spring Boot 微服务一站式启动器、生命周期管理器与交互式终端 UI（TUI）。

---

## 包含内容

```text
zed-java-launcher/
├── bin/
│   └── java-launcher                 # 预编译好的原生 CLI / TUI 核心二进制 (macOS arm64)
├── extension/                        # Zed 编辑器扩展目录
│   ├── extension.toml                # Zed 扩展清单文件
│   └── README.md                     # 扩展说明
├── install.sh                        # 自动化一键安装脚本
├── uninstall.sh                      # 卸载脚本
└── README.md                         # 本说明文档
```

---

## 安装方式

### 方式一：使用一键安装脚本（推荐）

在终端中进入解压后的目录并运行：

```bash
chmod +x install.sh
./install.sh
```

脚本将自动完成：
1. 安装可执行文件至 `~/.local/bin/java-launcher`；
2. 安装扩展至 `~/Library/Application Support/Zed/extensions/installed/java-launcher/`。

---

### 方式二：完全纯手动安装（零脚本依赖）

如果您希望手动复制文件，按以下 2 步操作即可：

#### 1. 安装核心可执行文件
```bash
# 复制到用户本地二进制目录
mkdir -p ~/.local/bin
cp bin/java-launcher ~/.local/bin/
chmod +x ~/.local/bin/java-launcher

# 确保 ~/.local/bin 在您的 PATH 中（可在 ~/.zshrc 或 ~/.bashrc 检查）
export PATH="$HOME/.local/bin:$PATH"
```

#### 2. 在 Zed 中加载扩展
您可以通过以下任意一种途径在 Zed 中载入扩展：

- **途径 A（GUI 命令面板）**：
  1. 打开 Zed 编辑器；
  2. 按快捷键 `Cmd+Shift+P` 打开命令面板；
  3. 输入并选择 **`zed: install dev extension`**；
  4. 在弹出的文件选择器中，选中本插件包内的 **`extension`** 文件夹即可。

- **途径 B（文件系统直接放置）**：
  直接将 `extension` 目录拷贝至 Zed 扩展目录：
  ```bash
  mkdir -p "$HOME/Library/Application Support/Zed/extensions/installed/java-launcher"
  cp -rf extension/* "$HOME/Library/Application Support/Zed/extensions/installed/java-launcher/"
  ```

---

## 在 Java 项目中启用

在您的 Maven / Spring Boot 项目根目录中运行一次同步命令：

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

1. 打开该项目，按下 `Cmd+Shift+P`；
2. 输入 **`task: spawn`** 并回车；
3. 选择 **`[Java Launcher] Open`** 即可唤起全功能 TUI 界面：
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
