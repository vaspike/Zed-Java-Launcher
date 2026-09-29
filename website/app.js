// ==========================================================================
// Java Launcher Official Website - Interactive Scripts & i18n
// ==========================================================================

const i18nData = {
  zh: {
    "nav.features": "核心特性",
    "nav.editions": "双端架构",
    "nav.comparison": "版本对比",
    "nav.simulator": "终端模拟",
    "nav.install": "快速安装",
    "github.zed": "Zed 仓库",
    "github.vscode": "VS Code 仓库",
    "hero.release_pill": "Dual Engine: 原生 Rust 内核 (Zed) + 全景可视化面板 (VS Code)",
    "hero.title_html": "现代 Java 微服务<br><span class=\"gradient-text\">零配置·开箱即用</span>",
    "hero.subtitle_html": "专为 Java & Spring Boot 开发者打造的轻量级微服务编排与调试利器。告别繁琐的 <code>launch.json</code> 与端口 <code>5005</code> 冲突，支持服务集群一键编排、动态调试端口分配与秒级项目解析。",
    "hero.btn_zed": "安装 Zed 官方插件",
    "hero.btn_vscode": "安装 VS Code 插件",
    "showcase.tab_zed": "⚡️ Zed Edition (Native Rust Engine)",
    "showcase.tab_vscode": "🔷 VS Code Edition (Interactive GUI)",
    "showcase.terminal_heading": "Terminal / Process Output",
    "features.tag": "Why Java Launcher?",
    "features.title": "重构 Java 本地微服务调试体验",
    "features.desc": "直击多微服务开发痛点，提供专为现代轻量级编辑器定制的启动与调试管线。",
    "features.f1_title": "动态 JDWP 端口隔离",
    "features.f1_desc_html": "告别 <code>Address already in use: 5005</code>！Java Launcher 自动为每个微服务分配并协商专属调试端口，支持 10+ 服务并发断点调试，零端口冲突。",
    "features.f2_title": "微服务一键编排 (Group Launch)",
    "features.f2_desc_html": "只需声明启动顺序与延时（如网关启动后延时 2s 启动订单服务），一键即可 <code>Group up</code>、<code>Group restart</code> 优雅重启或 <code>Group down</code> 批量停止。",
    "features.f3_title": "AST 毫秒级智能扫描",
    "features.f3_desc_html": "利用 Tree-sitter 高性能语法分析器与 Maven 反应堆模型，零 Java 进程开销，毫秒级扫描 <code>@SpringBootApplication</code>、<code>main</code> 入口与测试类。",
    "features.f4_title": "双向无缝迁移与互通",
    "features.f4_desc_html": "从 VS Code 迁移到 Zed？只需一条命令 <code>java-launcher init --from-vscode</code>，自动将旧配置与聚合启动无损转换为 Zed 专属配置。",
    "editions.tag": "Dual-Engine Architecture",
    "editions.title": "两种编辑器，同样的极速体验",
    "editions.desc": "针对 Zed 的极简键盘流与 VS Code 的可视化交互深度定制，量身打磨。",
    "editions.zed_badge": "Rust 原生内核 + WASM",
    "editions.zed_desc": "专为 Zed 编辑器量身打造。采用纯 Rust 编译的原生 CLI + WASM 扩展，内存占用低于 10MB，自动由 Zed 插件商店拉取运行。",
    "editions.zed_f1_html": "<strong>极速 Run 模式与 Debug 模式分离</strong>：日常运行无需挂载调试器，无任何额外 RAM 开销。",
    "editions.zed_f2_html": "<strong>Zed 专属 DAP 调试适配器</strong>：完全接管 Zed 调试面板，一键调试 <code>JL-Group-&lt;group&gt;</code>。",
    "editions.zed_f3_html": "<strong>自动补全 Schema</strong>：编辑 <code>config.json</code> 时自动弹出全工程可用服务下拉提示。",
    "editions.zed_f4_html": "<strong>洁癖模式 (Clean Mode)</strong>：防面板污染，避免数十个服务充斥 Zed 命令面板。",
    "editions.zed_f5_html": "<strong>安全自动备份与日志清空</strong>：修改配置自动时间戳备份，一键清空后台日志。",
    "editions.zed_btn": "探索 Zed 插件源码",
    "editions.vscode_badge": "全功能可视化交互",
    "editions.vscode_desc": "提供 Visual Studio Code 活动栏 (Activity Bar) 树状视图，通过图形化交互完成 Spring Profile 切换、聚合配置与进程管控。",
    "editions.vscode_f1_html": "<strong>Activity Bar 视图导航</strong>：一目了然列出所有 Spring Boot、Main 方法与单元测试。",
    "editions.vscode_f2_html": "<strong>Spring Active Profile 快速切换</strong>：支持单个服务或全工程批量设为 <code>dev</code>、<code>test</code>、<code>prod</code>。",
    "editions.vscode_f3_html": "<strong>JMX 远程管理集成</strong>：支持一键开关 JMX Remote，实时监控 JVM 运行指标。",
    "editions.vscode_f4_html": "<strong>可视化聚合启动器</strong>：图形化拖拽或右键将服务追加至聚合启动列表。",
    "editions.vscode_f5_html": "<strong>单元测试方法级定位</strong>：直接精准运行 JUnit 4/5 或 TestNG 单个测试用例。",
    "editions.vscode_btn": "探索 VS Code 插件源码",
    "comparison.tag": "Comparison Matrix",
    "comparison.title": "全方位特性对照",
    "comparison.desc": "看看 Java Launcher 相比传统 IDE 或手动配置能为你节省多少时间",
    "comparison.th_dimension": "能力维度",
    "comparison.th_manual": "传统手工配置 / 默认调试",
    "comparison.row1_dim": "入口自动发现",
    "comparison.row1_manual": "✗ 手动寻找类名与包路径",
    "comparison.row1_vscode": "✓ 侧边栏树形自动罗列",
    "comparison.row1_zed": "✓ AST 毫秒级全工程扫描",
    "comparison.row2_dim": "多微服务集群启动",
    "comparison.row2_manual": "✗ 开启 8-10 个终端标签页",
    "comparison.row2_vscode": "✓ 聚合启动 (Aggregated Launch)",
    "comparison.row2_zed": "✓ Group 编排 (支持顺序与毫秒延时)",
    "comparison.row3_dim": "调试端口冲突处理",
    "comparison.row3_manual": "✗ 频发 5005 Address in use",
    "comparison.row3_vscode": "✓ 多任务 launch.json 独立端口",
    "comparison.row3_zed": "✓ 动态 JDWP 端口协商与路由",
    "comparison.row4_dim": "底层架构与开销",
    "comparison.row4_manual": "— IDE 臃肿重度守护",
    "comparison.row4_vscode": "✓ TypeScript + VSCode API",
    "comparison.row4_zed": "✓ 纯 Rust 原生二进制 (<10MB)",
    "comparison.row5_dim": "免调试轻量运行 (Run)",
    "comparison.row5_manual": "△ 手写 java/mvn 脚本",
    "comparison.row5_vscode": "✓ 支持无调试器直接运行",
    "comparison.row5_zed": "✓ 零调试端口、原生终端直跑",
    "comparison.row6_dim": "构建系统支持",
    "comparison.row6_manual": "— 手动指定",
    "comparison.row6_vscode": "✓ Maven + Gradle",
    "comparison.row6_zed": "✓ Maven 多模块 Reactor",
    "simulator.tag": "Interactive Demo",
    "simulator.title": "在线体验命令行编排",
    "simulator.desc": "点击下方常用命令，查看 Java Launcher 引擎的实际执行流程与输出效果",
    "install.tag": "Get Started",
    "install.title": "30 秒快速上手",
    "install.desc": "挑选适合您工作流的安装方式",
    "install.tab_zed": "Zed 编辑器",
    "install.tab_vscode": "VS Code 编辑器",
    "install.tab_cli": "独立 CLI 二进制",
    "install.zed_s1_title": "从 Zed 插件中心安装",
    "install.zed_s1_desc_html": "打开 Zed，按 <code>Cmd+Shift+P</code> 搜索 <code>zed: extensions</code>，搜索 <strong>Java Launcher</strong> 并点击安装。",
    "install.zed_s2_title": "初始化工程配置",
    "install.zed_s2_desc_html": "在任意 Java / Spring Boot 工程中，按快捷键唤起命令并执行初始化：",
    "install.zed_s3_title": "一键运行或调试",
    "install.zed_s3_desc_html": "打开左侧 Debug 面板，选择 <code>JL-&lt;ServiceName&gt;</code> 或 <code>JL-Group-&lt;GroupName&gt;</code> 点击启动即可享受无冲突断点调试！",
    "install.vscode_s1_title": "从 VS Code 插件市场安装",
    "install.vscode_s1_desc_html": "打开 VS Code 扩展商店，搜索 <strong>Java Launcher</strong>，点击安装。",
    "install.vscode_s2_title": "使用 Activity Bar 树形面板",
    "install.vscode_s2_desc_html": "安装后活动栏将出现 Java Launcher 图标，自动扫描当前工程所有入口，点击服务名称右侧 ▶ 即可运行或配置聚合启动。",
    "install.cli_s1_title": "通过 Cargo 安装 (全平台)",
    "install.cli_s1_desc_html": "如果您已安装 Rust 工具链，可直接编译安装最新稳定版：",
    "install.cli_s2_title": "或下载 GitHub Releases 预编译包",
    "install.cli_s2_desc_html": "提供 macOS (Apple Silicon / Intel)、Linux (x86_64 / arm64) 以及 Windows 的独立预编译二进制压缩包。",
    "footer.desc": "为下一代轻量级开发者编辑器打造的 Java 微服务启动编排与调试引擎。",
    "footer.col_eco": "生态与仓库",
    "footer.col_nav": "快速导航",
    "footer.col_license": "开源与许可",
    "footer.copyright": "© 2026 Java Launcher. Open source under the MIT License. Crafted by <a href=\"https://github.com/vaspike\" target=\"_blank\" rel=\"noopener\" style=\"color: var(--accent-indigo); text-decoration: none;\">vaspike</a>.",
    "footer.tagline": "Built for speed, aesthetics, and developer happiness.",
    "copy.btn": "复制",
    "copy.copied": "✓ 已复制"
  },
  en: {
    "nav.features": "Features",
    "nav.editions": "Dual Architecture",
    "nav.comparison": "Comparison",
    "nav.simulator": "Terminal Demo",
    "nav.install": "Quick Start",
    "github.zed": "Zed Repo",
    "github.vscode": "VS Code Repo",
    "hero.release_pill": "Dual Engine: Native Rust (Zed) + Visual Explorer (VS Code)",
    "hero.title_html": "Modern Java Microservices,<br><span class=\"gradient-text\">Zero-Configuration Flow</span>",
    "hero.subtitle_html": "The lightweight microservice orchestrator and debugger built for Java & Spring Boot developers. Say goodbye to manual <code>launch.json</code> and port <code>5005</code> collisions with one-click group launches, dynamic debug ports, and instant project scanning.",
    "hero.btn_zed": "Get for Zed",
    "hero.btn_vscode": "Get for VS Code",
    "showcase.tab_zed": "⚡️ Zed Edition (Native Rust Engine)",
    "showcase.tab_vscode": "🔷 VS Code Edition (Interactive GUI)",
    "showcase.terminal_heading": "Terminal / Process Output",
    "features.tag": "Why Java Launcher?",
    "features.title": "Reimagining Local Java Microservices",
    "features.desc": "Eliminating multi-service development bottlenecks with custom pipelines tailored for modern lightweight editors.",
    "features.f1_title": "Dynamic JDWP Port Isolation",
    "features.f1_desc_html": "Say goodbye to <code>Address already in use: 5005</code>! Java Launcher automatically negotiates dedicated debug ports for each microservice, enabling 10+ concurrent debug sessions with zero collisions.",
    "features.f2_title": "Microservice Group Orchestration",
    "features.f2_desc_html": "Define launch sequences with millisecond delays (e.g., launch gateway, wait 2s, then launch order service). One-click <code>Group up</code>, graceful <code>Group restart</code>, and batch <code>Group down</code>.",
    "features.f3_title": "AST-Powered Instant Scanning",
    "features.f3_desc_html": "Leveraging Tree-sitter high-performance AST parsing and Maven reactor models. Zero Java process overhead; discovers <code>@SpringBootApplication</code>, <code>main</code> methods, and test suites in milliseconds.",
    "features.f4_title": "Two-Way Seamless Migration",
    "features.f4_desc_html": "Migrating from VS Code to Zed? Run <code>java-launcher init --from-vscode</code> to instantly import existing launch configurations and aggregated groups with zero data loss.",
    "editions.tag": "Dual-Engine Architecture",
    "editions.title": "Two Editors, Same Blazing Performance",
    "editions.desc": "Tailored specifically for Zed's minimalist keyboard flow and VS Code's rich visual interactivity.",
    "editions.zed_badge": "Native Rust Core + WASM",
    "editions.zed_desc": "Built for Zed. Powered by a native Rust CLI + WASM extension with under 10MB RAM footprint, auto-downloaded directly from the Zed Extension Store.",
    "editions.zed_f1_html": "<strong>Separation of Run & Debug Modes</strong>: Run dependencies without debugger overhead and minimal RAM footprint.",
    "editions.zed_f2_html": "<strong>Dedicated Zed DAP Debug Adapter</strong>: Integrates seamlessly with Zed's Debug panel for one-click <code>JL-Group-&lt;group&gt;</code> debugging.",
    "editions.zed_f3_html": "<strong>Auto-Completion Schema</strong>: Populates valid project services automatically as you type in <code>config.json</code>.",
    "editions.zed_f4_html": "<strong>Clean Mode (Prevent Palette Spam)</strong>: Keeps your Zed task palette tidy even in massive 50+ microservice projects.",
    "editions.zed_f5_html": "<strong>Safe Backups & Log Management</strong>: Timestamped backups before config edits, plus one-click safe log truncation.",
    "editions.zed_btn": "Explore Zed Repo",
    "editions.vscode_badge": "Full Visual Explorer",
    "editions.vscode_desc": "Offers an Activity Bar tree view for VS Code, enabling visual Spring Profile switching, aggregated configuration, and runtime process control.",
    "editions.vscode_f1_html": "<strong>Activity Bar Explorer</strong>: Lists all Spring Boot services, Main classes, and unit tests at a glance.",
    "editions.vscode_f2_html": "<strong>Fast Spring Active Profile Switching</strong>: Switch profiles per service or in bulk across your entire workspace.",
    "editions.vscode_f3_html": "<strong>JMX Remote Management</strong>: Toggle JMX Remote status to monitor live JVM metrics effortlessly.",
    "editions.vscode_f4_html": "<strong>Visual Aggregated Launcher</strong>: Add and organize services into aggregated launch configs with a single click.",
    "editions.vscode_f5_html": "<strong>Method-Level Test Runner</strong>: Run individual JUnit 4/5 or TestNG test methods directly from the tree view.",
    "editions.vscode_btn": "Explore VS Code Repo",
    "comparison.tag": "Comparison Matrix",
    "comparison.title": "Feature Comparison Matrix",
    "comparison.desc": "See how much time Java Launcher saves you compared to traditional manual setup or standard IDEs",
    "comparison.th_dimension": "Capability Dimension",
    "comparison.th_manual": "Manual Setup / Default Debug",
    "comparison.row1_dim": "Entry Point Discovery",
    "comparison.row1_manual": "✗ Manual search for class/package names",
    "comparison.row1_vscode": "✓ Activity Bar tree view auto-populates",
    "comparison.row1_zed": "✓ Millisecond AST full-workspace scan",
    "comparison.row2_dim": "Microservice Cluster Launch",
    "comparison.row2_manual": "✗ Open 8-10 terminal tabs manually",
    "comparison.row2_vscode": "✓ Aggregated Launch with delays",
    "comparison.row2_zed": "✓ Group Orchestration with ms delay",
    "comparison.row3_dim": "Debug Port Collision Handling",
    "comparison.row3_manual": "✗ Frequent 5005 Address in use crash",
    "comparison.row3_vscode": "✓ Multi-target isolated ports",
    "comparison.row3_zed": "✓ Dynamic JDWP port routing",
    "comparison.row4_dim": "Underlying Architecture & RAM",
    "comparison.row4_manual": "— Heavy background IDE daemons",
    "comparison.row4_vscode": "✓ TypeScript + VS Code API",
    "comparison.row4_zed": "✓ Native Rust Binary (<10MB)",
    "comparison.row5_dim": "Lightweight Run Mode (No Debugger)",
    "comparison.row5_manual": "△ Hand-crafted java/mvn scripts",
    "comparison.row5_vscode": "✓ Run directly without debugger",
    "comparison.row5_zed": "✓ Zero debug port, native terminal direct run",
    "comparison.row6_dim": "Build Tool Support",
    "comparison.row6_manual": "— Manual configuration",
    "comparison.row6_vscode": "✓ Maven + Gradle",
    "comparison.row6_zed": "✓ Maven Multi-Module Reactor",
    "simulator.tag": "Interactive Demo",
    "simulator.title": "Try the CLI Engine in Your Browser",
    "simulator.desc": "Click below to simulate real-time output and execution flow of the Java Launcher engine",
    "install.tag": "Get Started",
    "install.title": "Get Started in 30 Seconds",
    "install.desc": "Choose the installation method that fits your workflow",
    "install.tab_zed": "Zed Editor",
    "install.tab_vscode": "VS Code Editor",
    "install.tab_cli": "Standalone CLI",
    "install.zed_s1_title": "Install from Zed Extensions Store",
    "install.zed_s1_desc_html": "Open Zed, press <code>Cmd+Shift+P</code>, search <code>zed: extensions</code>, find <strong>Java Launcher</strong> and click Install.",
    "install.zed_s2_title": "Initialize Project Config",
    "install.zed_s2_desc_html": "In any Java / Spring Boot workspace, spawn the initialize task:",
    "install.zed_s3_title": "One-Click Run or Debug",
    "install.zed_s3_desc_html": "Open the Debug panel, select <code>JL-&lt;ServiceName&gt;</code> or <code>JL-Group-&lt;GroupName&gt;</code>, and click play to enjoy conflict-free debugging!",
    "install.vscode_s1_title": "Install from VS Code Marketplace",
    "install.vscode_s1_desc_html": "Open the VS Code Extensions view, search for <strong>Java Launcher</strong>, and click Install.",
    "install.vscode_s2_title": "Use the Activity Bar Tree View",
    "install.vscode_s2_desc_html": "Click the Java Launcher icon in the Activity Bar. It auto-scans all entries; click ▶ to run or manage aggregated configurations.",
    "install.cli_s1_title": "Install via Cargo (Cross-Platform)",
    "install.cli_s1_desc_html": "If you have the Rust toolchain installed, install directly via Cargo:",
    "install.cli_s2_title": "Or Download Precompiled Binaries from GitHub",
    "install.cli_s2_desc_html": "Precompiled standalone binary archives for macOS (Apple Silicon & Intel), Linux (x86_64 & arm64), and Windows.",
    "footer.desc": "The next-generation microservice orchestrator and debugger for modern lightweight developer editors.",
    "footer.col_eco": "Ecosystem & Repos",
    "footer.col_nav": "Quick Links",
    "footer.col_license": "Open Source & License",
    "footer.copyright": "© 2026 Java Launcher. Open source under the MIT License. Crafted by <a href=\"https://github.com/vaspike\" target=\"_blank\" rel=\"noopener\" style=\"color: var(--accent-indigo); text-decoration: none;\">vaspike</a>.",
    "footer.tagline": "Built for speed, aesthetics, and developer happiness.",
    "copy.btn": "Copy",
    "copy.copied": "✓ Copied"
  }
};

let currentLang = 'zh';

document.addEventListener('DOMContentLoaded', () => {
  initLanguageToggle();
  initThemeToggle();
  initShowcaseSwitcher();
  initTerminalSimulator();
  initInstallationTabs();
  initCopyButtons();
});

/* --------------------------------------------------------------------------
   Language Toggle & Translation Logic
   -------------------------------------------------------------------------- */
function initLanguageToggle() {
  const langBtn = document.getElementById('langToggleBtn');
  const savedLang = localStorage.getItem('jl-lang') || 
    (navigator.language.startsWith('zh') ? 'zh' : 'en');

  currentLang = savedLang;
  applyLanguage(currentLang);

  if (langBtn) {
    langBtn.addEventListener('click', () => {
      currentLang = currentLang === 'zh' ? 'en' : 'zh';
      localStorage.setItem('jl-lang', currentLang);
      applyLanguage(currentLang);
    });
  }
}

function applyLanguage(lang) {
  const dict = i18nData[lang] || i18nData.zh;
  document.documentElement.setAttribute('lang', lang === 'zh' ? 'zh-CN' : 'en');

  // Update language button label (shows the OTHER language or current indicator)
  const langLabel = document.getElementById('langLabel');
  if (langLabel) {
    langLabel.textContent = lang === 'zh' ? 'EN' : '中';
  }

  // Update plain text elements
  document.querySelectorAll('[data-i18n]').forEach(el => {
    const key = el.getAttribute('data-i18n');
    if (dict[key]) {
      el.textContent = dict[key];
    }
  });

  // Update HTML elements (containing markup like <br>, <strong>, <code>)
  document.querySelectorAll('[data-i18n-html]').forEach(el => {
    const key = el.getAttribute('data-i18n-html');
    if (dict[key]) {
      el.innerHTML = dict[key];
    }
  });
}

/* --------------------------------------------------------------------------
   Theme Switcher (Dark / Light with LocalStorage)
   -------------------------------------------------------------------------- */
function initThemeToggle() {
  const toggleBtn = document.getElementById('themeToggleBtn');
  if (!toggleBtn) return;

  const savedTheme = localStorage.getItem('jl-theme') || 
    (window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark');
  
  applyTheme(savedTheme);

  toggleBtn.addEventListener('click', () => {
    const currentTheme = document.documentElement.getAttribute('data-theme') || 'dark';
    const nextTheme = currentTheme === 'dark' ? 'light' : 'dark';
    applyTheme(nextTheme);
    localStorage.setItem('jl-theme', nextTheme);
  });
}

function applyTheme(theme) {
  document.documentElement.setAttribute('data-theme', theme);
  const toggleBtn = document.getElementById('themeToggleBtn');
  if (toggleBtn) {
    toggleBtn.innerHTML = theme === 'dark' 
      ? `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="5"></circle><line x1="12" y1="1" x2="12" y2="3"></line><line x1="12" y1="21" x2="12" y2="23"></line><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"></line><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"></line><line x1="1" y1="12" x2="3" y2="12"></line><line x1="21" y1="12" x2="23" y2="12"></line><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"></line><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"></line></svg>`
      : `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"></path></svg>`;
  }
}

/* --------------------------------------------------------------------------
   Hero Showcase Editor Switcher (Zed vs VS Code)
   -------------------------------------------------------------------------- */
const showcaseData = {
  zed: {
    badge: 'Zed Edition',
    badgeClass: 'zed-badge',
    title: 'zed-java-project — .java-launcher/config.json',
    tabs: [
      { name: 'config.json', active: true },
      { name: '.zed/tasks.json', active: false },
      { name: '.zed/debug.json', active: false }
    ],
    sidebarHeading: 'Zed Worktree & Tasks',
    sidebarItems: [
      { icon: '📁', name: '.java-launcher', badge: 'config', badgeClass: 'badge-group' },
      { icon: '⚡', name: '[JL] Group up cluster', badge: 'task', badgeClass: 'badge-dev' },
      { icon: '🐛', name: 'JL-Group-cluster', badge: 'DAP', badgeClass: 'badge-prod' },
      { icon: '📦', name: 'gateway-service', badge: '8080', badgeClass: 'badge-dev' },
      { icon: '📦', name: 'order-service', badge: '8081', badgeClass: 'badge-dev' },
      { icon: '📦', name: 'payment-service', badge: '8082', badgeClass: 'badge-dev' }
    ],
    code: `{
  <span class="token-key">"$schema"</span>: <span class="token-str">"./config.schema.json"</span>,
  <span class="token-key">"version"</span>: <span class="token-num">1</span>,
  <span class="token-key">"groups"</span>: {
    <span class="token-key">"microservice-cluster"</span>: [
      {
        <span class="token-key">"entry"</span>: <span class="token-str">"gateway::com.example.GatewayApplication"</span>,
        <span class="token-key">"delay_ms"</span>: <span class="token-num">0</span>,    <span class="token-comment">// Start gateway immediately</span>
        <span class="token-key">"enabled"</span>: <span class="token-bool">true</span>
      },
      {
        <span class="token-key">"entry"</span>: <span class="token-str">"order-service::com.example.OrderApplication"</span>,
        <span class="token-key">"delay_ms"</span>: <span class="token-num">2000</span>, <span class="token-comment">// Wait 2s for discovery registration</span>
        <span class="token-key">"enabled"</span>: <span class="token-bool">true</span>
      },
      {
        <span class="token-key">"entry"</span>: <span class="token-str">"payment-service::com.example.PaymentApplication"</span>,
        <span class="token-key">"delay_ms"</span>: <span class="token-num">1000</span>,
        <span class="token-key">"enabled"</span>: <span class="token-bool">true</span>
      }
    ]
  }
}`,
    terminalLines: [
      '<span class="terminal-prompt">$</span> java-launcher group up microservice-cluster',
      '<span class="terminal-accent">[INFO]</span> Scanning Maven multi-module reactor (3 modules found in 42ms)...',
      '<span class="terminal-accent">[EXEC]</span> [1/3] Starting gateway (port 8080, JDWP 5005)... PID: 82914',
      '<span class="terminal-accent">[WAIT]</span> Sleeping 2000ms for gateway warmup...',
      '<span class="terminal-accent">[EXEC]</span> [2/3] Starting order-service (port 8081, JDWP 5006)... PID: 82950',
      '<span class="terminal-accent">[WAIT]</span> Sleeping 1000ms...',
      '<span class="terminal-accent">[EXEC]</span> [3/3] Starting payment-service (port 8082, JDWP 5007)... PID: 82982',
      '<span class="terminal-success">✔ All 3 microservices launched successfully with 0 port collisions!</span>'
    ]
  },
  vscode: {
    badge: 'VS Code Edition',
    badgeClass: 'vscode-badge',
    title: 'vscode-java-project — .vscode/aggregated-launch.json',
    tabs: [
      { name: 'aggregated-launch.json', active: true },
      { name: 'launch.json', active: false },
      { name: 'Spring Profiles', active: false }
    ],
    sidebarHeading: 'Java Launcher Explorer',
    sidebarItems: [
      { icon: '🚀', name: 'Aggregated: Cloud-Cluster', badge: 'Run', badgeClass: 'badge-group' },
      { icon: '🍃', name: 'GatewayApplication', badge: 'dev', badgeClass: 'badge-dev' },
      { icon: '🍃', name: 'OrderServiceApplication', badge: 'dev', badgeClass: 'badge-dev' },
      { icon: '🍃', name: 'PaymentServiceApp', badge: 'prod', badgeClass: 'badge-prod' },
      { icon: '🧪', name: 'UserServiceTest (JUnit 5)', badge: 'test', badgeClass: 'badge-group' },
      { icon: '📊', name: 'JMX Remote Management', badge: 'on', badgeClass: 'badge-dev' }
    ],
    code: `{
  <span class="token-key">"version"</span>: <span class="token-str">"0.2.0"</span>,
  <span class="token-key">"configurations"</span>: [
    {
      <span class="token-key">"type"</span>: <span class="token-str">"java-aggregated"</span>,
      <span class="token-key">"name"</span>: <span class="token-str">"Launch Cloud Microservices"</span>,
      <span class="token-key">"request"</span>: <span class="token-str">"launch"</span>,
      <span class="token-key">"services"</span>: [
        {
          <span class="token-key">"name"</span>: <span class="token-str">"Gateway Application"</span>,
          <span class="token-key">"profile"</span>: <span class="token-str">"dev"</span>,
          <span class="token-key">"delaySeconds"</span>: <span class="token-num">0</span>
        },
        {
          <span class="token-key">"name"</span>: <span class="token-str">"Order Service"</span>,
          <span class="token-key">"profile"</span>: <span class="token-str">"dev"</span>,
          <span class="token-key">"delaySeconds"</span>: <span class="token-num">2</span>
        }
      ]
    }
  ]
}`,
    terminalLines: [
      '<span class="terminal-prompt">[VSCode Java Launcher]</span> Initialized Spring Boot Discovery Service...',
      '<span class="terminal-accent">[ACTIVITY]</span> Populating TreeView with 14 detected entry points',
      '<span class="terminal-accent">[PROFILE]</span> Set active profile: dev for 8 services',
      '<span class="terminal-accent">[JMX]</span> Remote management listening on 127.0.0.1:9010',
      '<span class="terminal-success">✔ Ready. Click ▶ on Activity Bar to run aggregated cluster.</span>'
    ]
  }
};

function initShowcaseSwitcher() {
  const tabZed = document.getElementById('tabZed');
  const tabVSCode = document.getElementById('tabVSCode');
  if (!tabZed || !tabVSCode) return;

  tabZed.addEventListener('click', () => setEditorShowcase('zed'));
  tabVSCode.addEventListener('click', () => setEditorShowcase('vscode'));

  setEditorShowcase('zed');
}

function setEditorShowcase(mode) {
  const data = showcaseData[mode];
  if (!data) return;

  const tabZed = document.getElementById('tabZed');
  const tabVSCode = document.getElementById('tabVSCode');

  if (mode === 'zed') {
    tabZed.classList.add('active-zed');
    tabVSCode.classList.remove('active-vscode');
  } else {
    tabZed.classList.remove('active-zed');
    tabVSCode.classList.add('active-vscode');
  }

  const badgeEl = document.getElementById('windowBadge');
  const titleEl = document.getElementById('windowTitle');
  if (badgeEl) {
    badgeEl.textContent = data.badge;
    badgeEl.className = `window-badge ${data.badgeClass}`;
  }
  if (titleEl) {
    titleEl.textContent = data.title;
  }

  const tabsContainer = document.getElementById('editorTabs');
  if (tabsContainer) {
    tabsContainer.innerHTML = data.tabs.map(tab => `
      <div class="editor-tab ${tab.active ? 'active' : ''}">
        <span>📄</span> ${tab.name}
      </div>
    `).join('');
  }

  const sidebarHeading = document.getElementById('sidebarHeading');
  const sidebarTree = document.getElementById('sidebarTree');
  if (sidebarHeading) sidebarHeading.textContent = data.sidebarHeading;
  if (sidebarTree) {
    sidebarTree.innerHTML = data.sidebarItems.map(item => `
      <li class="tree-item">
        <span>${item.icon}</span>
        <span>${item.name}</span>
        <span class="badge-pill ${item.badgeClass}">${item.badge}</span>
      </li>
    `).join('');
  }

  const codeView = document.getElementById('codeView');
  if (codeView) {
    codeView.innerHTML = `<pre><code>${data.code}</code></pre>`;
  }

  const terminalLines = document.getElementById('terminalLines');
  if (terminalLines) {
    terminalLines.innerHTML = data.terminalLines.map(line => `
      <div class="terminal-line">${line}</div>
    `).join('');
  }
}

/* --------------------------------------------------------------------------
   Interactive Terminal Simulator
   -------------------------------------------------------------------------- */
const terminalCommands = {
  scan: {
    command: 'java-launcher scan --all',
    output: [
      '<span class="terminal-prompt">$ java-launcher scan --all</span>',
      '<span class="terminal-accent">[INFO]</span> Resolving Maven reactor in /workspace/spring-cloud-demo...',
      '<span class="terminal-accent">[SCAN]</span> Analyzed 68 Java source files with tree-sitter in 38ms',
      '<span class="terminal-success">✔ Discovered 4 Spring Boot Applications:</span>',
      '  • gateway             -> com.example.cloud.GatewayApplication',
      '  • user-service        -> com.example.cloud.UserApplication',
      '  • order-service       -> com.example.cloud.OrderApplication',
      '  • notification-app    -> com.example.cloud.NotificationApplication',
      '<span class="terminal-success">✔ Discovered 12 Test Suites (JUnit 5 & TestNG)</span>'
    ]
  },
  init: {
    command: 'java-launcher init --write',
    output: [
      '<span class="terminal-prompt">$ java-launcher init --write</span>',
      '<span class="terminal-accent">[CREATE]</span> .java-launcher/config.json with default groups',
      '<span class="terminal-accent">[CREATE]</span> .java-launcher/config.schema.json (enables Zed auto-completion)',
      '<span class="terminal-accent">[MERGE]</span> .zed/tasks.json (added [Java Launcher] group tasks)',
      '<span class="terminal-accent">[MERGE]</span> .zed/debug.json (registered JL DAP debug targets)',
      '<span class="terminal-success">✔ Initialized successfully! Open Zed Debug Panel or Command Palette to run.</span>'
    ]
  },
  group: {
    command: 'java-launcher group up dev-cluster',
    output: [
      '<span class="terminal-prompt">$ java-launcher group up dev-cluster</span>',
      '<span class="terminal-accent">[1/3]</span> Starting gateway-service on port 8080 (debug port: 5005)... PID: 39102',
      '<span class="terminal-accent">[WAIT]</span> Delaying 2000ms before next service...',
      '<span class="terminal-accent">[2/3]</span> Starting order-service on port 8081 (debug port: 5006)... PID: 39145',
      '<span class="terminal-accent">[WAIT]</span> Delaying 1000ms...',
      '<span class="terminal-accent">[3/3]</span> Starting user-service on port 8082 (debug port: 5007)... PID: 39180',
      '<span class="terminal-success">✔ Group dev-cluster is LIVE. All ports isolated & collision-free.</span>'
    ]
  },
  dap: {
    command: 'java-launcher dap --name "JL-Group-dev-cluster"',
    output: [
      '<span class="terminal-prompt">$ java-launcher dap --name "JL-Group-dev-cluster"</span>',
      '<span class="terminal-accent">[DAP]</span> Negotiated Zed Debug Adapter Protocol handshake',
      '<span class="terminal-accent">[DAP]</span> Dynamic JDWP port routing initialized:',
      '  ├── gateway-service  -> 127.0.0.1:5005 (Connected)',
      '  ├── order-service    -> 127.0.0.1:5006 (Connected)',
      '  └── user-service     -> 127.0.0.1:5007 (Connected)',
      '<span class="terminal-success">✔ Concurrently debugging 3 microservices in single Zed Debug session!</span>'
    ]
  }
};

function initTerminalSimulator() {
  const buttons = document.querySelectorAll('.cmd-pill');
  const viewport = document.getElementById('simTerminalViewport');
  if (!buttons.length || !viewport) return;

  buttons.forEach(btn => {
    btn.addEventListener('click', () => {
      buttons.forEach(b => b.classList.remove('active'));
      btn.classList.add('active');

      const cmdKey = btn.getAttribute('data-cmd');
      const cmdData = terminalCommands[cmdKey];
      if (!cmdData) return;

      runSimulatedCommand(viewport, cmdData.output);
    });
  });
}

function runSimulatedCommand(viewport, lines) {
  viewport.innerHTML = '<div class="terminal-line"><span class="terminal-prompt">Executing...</span></div>';
  let index = 0;

  viewport.innerHTML = '';
  function appendNext() {
    if (index < lines.length) {
      const lineDiv = document.createElement('div');
      lineDiv.className = 'terminal-line';
      lineDiv.innerHTML = lines[index];
      viewport.appendChild(lineDiv);
      viewport.scrollTop = viewport.scrollHeight;
      index++;
      setTimeout(appendNext, 110);
    }
  }
  appendNext();
}

/* --------------------------------------------------------------------------
   Installation Tabs Switching
   -------------------------------------------------------------------------- */
function initInstallationTabs() {
  const tabs = document.querySelectorAll('.install-tab-btn');
  const panels = document.querySelectorAll('.install-panel');

  tabs.forEach(tab => {
    tab.addEventListener('click', () => {
      const targetId = tab.getAttribute('data-target');
      
      tabs.forEach(t => t.classList.remove('active'));
      panels.forEach(p => p.classList.remove('active'));

      tab.classList.add('active');
      const targetPanel = document.getElementById(targetId);
      if (targetPanel) targetPanel.classList.add('active');
    });
  });
}

/* --------------------------------------------------------------------------
   Copy to Clipboard & Toast
   -------------------------------------------------------------------------- */
function initCopyButtons() {
  const copyButtons = document.querySelectorAll('.copy-btn');

  copyButtons.forEach(btn => {
    btn.addEventListener('click', () => {
      const textToCopy = btn.getAttribute('data-copy');
      if (!textToCopy) return;

      navigator.clipboard.writeText(textToCopy).then(() => {
        const copiedText = currentLang === 'zh' ? '✓ 已复制' : '✓ Copied';
        const defaultText = currentLang === 'zh' ? '复制' : 'Copy';
        showToast(currentLang === 'zh' ? '已复制到剪贴板！' : 'Copied to clipboard!');
        btn.textContent = copiedText;
        setTimeout(() => {
          btn.textContent = defaultText;
        }, 2000);
      });
    });
  });
}

function showToast(message) {
  const toast = document.getElementById('toastNotification');
  if (!toast) return;

  toast.textContent = message;
  toast.classList.add('show');
  setTimeout(() => {
    toast.classList.remove('show');
  }, 2200);
}
