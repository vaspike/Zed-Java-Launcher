#!/usr/bin/env bash
set -euo pipefail

echo "=========================================="
echo "    Zed Java Launcher Uninstaller"
echo "=========================================="

if [[ -f "${HOME}/.local/bin/java-launcher" ]]; then
    echo "-> Removing ${HOME}/.local/bin/java-launcher..."
    rm -f "${HOME}/.local/bin/java-launcher"
fi

ZED_EXT_DIR="${HOME}/Library/Application Support/Zed/extensions/installed/zed-java-launcher"
if [[ -d "${ZED_EXT_DIR}" ]]; then
    echo "-> Removing Zed extension from ${ZED_EXT_DIR}..."
    rm -rf "${ZED_EXT_DIR}"
fi

OLD_EXT_DIR="${HOME}/Library/Application Support/Zed/extensions/installed/java-launcher"
if [[ -d "${OLD_EXT_DIR}" ]]; then
    rm -rf "${OLD_EXT_DIR}"
fi

# Clean up global tasks from ~/.config/zed/tasks.json if present
ZED_TASKS_FILE="${HOME}/.config/zed/tasks.json"
if [[ -f "${ZED_TASKS_FILE}" ]] && command -v python3 >/dev/null 2>&1; then
    echo "-> Cleaning up global tasks from ${ZED_TASKS_FILE}..."
    python3 - << 'PYEOF'
import json, os

tasks_file = os.path.expanduser("~/.config/zed/tasks.json")
if os.path.exists(tasks_file):
    try:
        with open(tasks_file, "r", encoding="utf-8") as f:
            content = f.read().strip()
            if content:
                lines = [l for l in content.splitlines() if not l.strip().startswith("//")]
                tasks = json.loads("\n".join(lines))
                if isinstance(tasks, list):
                    filtered = [t for t in tasks if not (isinstance(t, dict) and t.get("label", "").startswith("Java Launcher:"))]
                    with open(tasks_file, "w", encoding="utf-8") as out_f:
                        json.dump(filtered, out_f, indent=2, ensure_ascii=False)
                        out_f.write("\n")
    except Exception:
        pass
PYEOF
fi

echo ""
echo "✅ Zed Java Launcher has been uninstalled."
echo "=========================================="
