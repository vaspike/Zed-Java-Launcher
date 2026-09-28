#!/usr/bin/env bash
set -euo pipefail

echo "=========================================="
echo "    Zed Java Launcher Uninstaller"
echo "=========================================="

# Remove binary
if [[ -f "${HOME}/.local/bin/java-launcher" ]]; then
    echo "-> Removing ${HOME}/.local/bin/java-launcher..."
    rm -f "${HOME}/.local/bin/java-launcher"
fi

# Remove Zed extension
ZED_EXT_DIR="${HOME}/Library/Application Support/Zed/extensions/installed/java-launcher"
if [[ -d "${ZED_EXT_DIR}" ]]; then
    echo "-> Removing Zed extension from ${ZED_EXT_DIR}..."
    rm -rf "${ZED_EXT_DIR}"
fi

echo ""
echo "✅ Zed Java Launcher has been uninstalled."
echo "=========================================="
