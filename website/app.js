// ==========================================================================
// Java Launcher Official Website - Interactive Scripts
// ==========================================================================

document.addEventListener('DOMContentLoaded', () => {
  initThemeToggle();
  initShowcaseSwitcher();
  initTerminalSimulator();
  initInstallationTabs();
  initCopyButtons();
});

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
      ? `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="5"></circle><line x1="12" y1="1" x2="12" y2="3"></line><line x1="12" y1="21" x2="12" y2="23"></line><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"></line><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"></line><line x1="1" y1="12" x2="3" y2="12"></line><line x1="21" y1="12" x2="23" y2="12"></line><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"></line><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"></line></svg>`
      : `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"></path></svg>`;
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

  // Update window title & badge
  const badgeEl = document.getElementById('windowBadge');
  const titleEl = document.getElementById('windowTitle');
  if (badgeEl) {
    badgeEl.textContent = data.badge;
    badgeEl.className = `window-badge ${data.badgeClass}`;
  }
  if (titleEl) {
    titleEl.textContent = data.title;
  }

  // Update tabs
  const tabsContainer = document.getElementById('editorTabs');
  if (tabsContainer) {
    tabsContainer.innerHTML = data.tabs.map(tab => `
      <div class="editor-tab ${tab.active ? 'active' : ''}">
        <span>📄</span> ${tab.name}
      </div>
    `).join('');
  }

  // Update sidebar
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

  // Update code view
  const codeView = document.getElementById('codeView');
  if (codeView) {
    codeView.innerHTML = `<pre><code>${data.code}</code></pre>`;
  }

  // Update terminal
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
      setTimeout(appendNext, 120);
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
  const toast = document.getElementById('toastNotification');

  copyButtons.forEach(btn => {
    btn.addEventListener('click', () => {
      const textToCopy = btn.getAttribute('data-copy');
      if (!textToCopy) return;

      navigator.clipboard.writeText(textToCopy).then(() => {
        showToast('Copied to clipboard!');
        const originalText = btn.innerHTML;
        btn.innerHTML = '✓ Copied';
        setTimeout(() => {
          btn.innerHTML = originalText;
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
