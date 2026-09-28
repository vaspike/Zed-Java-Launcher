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

# Also clean up old folder name if present
OLD_EXT_DIR="${HOME}/Library/Application Support/Zed/extensions/installed/java-launcher"
if [[ -d "${OLD_EXT_DIR}" ]]; then
    rm -rf "${OLD_EXT_DIR}"
fi

echo ""
echo "✅ Zed Java Launcher has been uninstalled."
echo "=========================================="
