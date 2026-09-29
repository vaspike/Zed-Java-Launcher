#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_SRC="${SCRIPT_DIR}/bin/java-launcher"
EXT_TOML="${SCRIPT_DIR}/extension.toml"
EXT_WASM="${SCRIPT_DIR}/extension.wasm"

echo "=========================================="
echo "    Zed Java Launcher Installer"
echo "=========================================="

if [[ ! -f "${BIN_SRC}" ]]; then
    echo "❌ Error: Missing binary file at ${BIN_SRC}" >&2
    exit 1
fi

if [[ ! -f "${EXT_TOML}" ]]; then
    echo "❌ Error: Missing extension manifest at ${EXT_TOML}" >&2
    exit 1
fi

# 1. Install CLI binary to ~/.local/bin
INSTALL_BIN_DIR="${HOME}/.local/bin"
mkdir -p "${INSTALL_BIN_DIR}"
echo "-> [1/3] Installing CLI binary to ${INSTALL_BIN_DIR}/java-launcher..."
cp -f "${BIN_SRC}" "${INSTALL_BIN_DIR}/java-launcher"
chmod +x "${INSTALL_BIN_DIR}/java-launcher"

# Check if ~/.local/bin is in PATH
if [[ ":$PATH:" != *":${INSTALL_BIN_DIR}:"* ]]; then
    echo ""
    echo "⚠️  NOTE: ${INSTALL_BIN_DIR} is not currently in your PATH environment variable."
    echo "   To use 'java-launcher' globally from any terminal, add this to your ~/.zshrc or ~/.bashrc:"
    echo "     export PATH=\"\${HOME}/.local/bin:\$PATH\""
    echo ""
fi

# 2. Install Zed extension directly into Zed's extensions directory
ZED_EXT_DIR="${HOME}/Library/Application Support/Zed/extensions/installed/zed-java-launcher"
echo "-> [2/3] Installing Zed extension to:"
echo "   ${ZED_EXT_DIR}..."
rm -rf "${ZED_EXT_DIR}"
mkdir -p "${ZED_EXT_DIR}"
cp -f "${EXT_TOML}" "${ZED_EXT_DIR}/extension.toml"
if [[ -f "${EXT_WASM}" ]]; then
    cp -f "${EXT_WASM}" "${ZED_EXT_DIR}/extension.wasm"
fi
if [[ -f "${SCRIPT_DIR}/README.md" ]]; then
    cp -f "${SCRIPT_DIR}/README.md" "${ZED_EXT_DIR}/README.md"
fi

# 3. Register Global Bootstrap Tasks in Zed (~/.config/zed/tasks.json)
ZED_CONFIG_DIR="${HOME}/.config/zed"
ZED_TASKS_FILE="${ZED_CONFIG_DIR}/tasks.json"
mkdir -p "${ZED_CONFIG_DIR}"

echo "-> [3/3] Registering global bootstrap tasks in ${ZED_TASKS_FILE}..."

if command -v python3 >/dev/null 2>&1; then
    python3 - << 'PYEOF'
import json, os

config_dir = os.path.expanduser("~/.config/zed")
tasks_file = os.path.join(config_dir, "tasks.json")

new_tasks = [
    {
        "label": "[Java Launcher] Initialize Project",
        "command": "java-launcher",
        "args": ["sync-zed", "--write"],
        "use_new_terminal": False,
        "allow_concurrent_runs": False
    }
]

existing_tasks = []
if os.path.exists(tasks_file):
    try:
        with open(tasks_file, "r", encoding="utf-8") as f:
            content = f.read().strip()
            if content:
                lines = [l for l in content.splitlines() if not l.strip().startswith("//")]
                parsed = json.loads("\n".join(lines))
                if isinstance(parsed, list):
                    existing_tasks = parsed
    except Exception as e:
        print(f"   Notice: Existing tasks.json could not be parsed: {e}")

# Clean up obsolete legacy bootstrap tasks
cleaned_tasks = [
    t for t in existing_tasks
    if not (isinstance(t, dict) and t.get("label", "").startswith("Java Launcher:"))
]

task_map = {t["label"]: t for t in new_tasks}
merged = []
seen = set()

for t in cleaned_tasks:
    if isinstance(t, dict) and "label" in t:
        lbl = t["label"]
        if lbl in task_map:
            merged.append(task_map[lbl])
            seen.add(lbl)
        else:
            merged.append(t)
    else:
        merged.append(t)

for nt in new_tasks:
    if nt["label"] not in seen:
        merged.append(nt)

with open(tasks_file, "w", encoding="utf-8") as f:
    json.dump(merged, f, indent=2, ensure_ascii=False)
    f.write("\n")
PYEOF
else
    if [[ ! -f "${ZED_TASKS_FILE}" ]]; then
        cat << 'EOF' > "${ZED_TASKS_FILE}"
[
  {
    "label": "[Java Launcher] Initialize Project",
    "command": "java-launcher",
    "args": ["sync-zed", "--write"],
    "use_new_terminal": false,
    "allow_concurrent_runs": false
  }
]
EOF
    fi
fi

echo ""
echo "=========================================="
echo "  ✅ Installation completed successfully!"
echo "=========================================="
echo ""
echo "Quick Start inside Zed (Zero Command Line Needed!):"
echo "1. Open any Java / Spring Boot project in Zed."
echo "2. Press Cmd+Shift+P -> search 'task: spawn' (or press Ctrl+T)."
echo "3. Run '[Java Launcher] Initialize Project'."
echo "4. Done! All project tasks and debug configurations are now live in Zed."
echo "=========================================="
