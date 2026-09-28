#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_SRC="${SCRIPT_DIR}/bin/java-launcher"
EXT_SRC="${SCRIPT_DIR}/extension"

echo "=========================================="
echo "    Zed Java Launcher Installer"
echo "=========================================="

if [[ ! -f "${BIN_SRC}" ]]; then
    echo "❌ Error: Missing binary file at ${BIN_SRC}" >&2
    exit 1
fi

if [[ ! -d "${EXT_SRC}" ]]; then
    echo "❌ Error: Missing extension directory at ${EXT_SRC}" >&2
    exit 1
fi

# 1. Install CLI binary to ~/.local/bin
INSTALL_BIN_DIR="${HOME}/.local/bin"
mkdir -p "${INSTALL_BIN_DIR}"
echo "-> [1/2] Installing CLI binary to ${INSTALL_BIN_DIR}/java-launcher..."
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

# 2. Install Zed extension into Zed extensions directory
ZED_EXT_DIR="${HOME}/Library/Application Support/Zed/extensions/installed/java-launcher"
echo "-> [2/2] Installing Zed extension to:"
echo "   ${ZED_EXT_DIR}..."
mkdir -p "${ZED_EXT_DIR}"
cp -rf "${EXT_SRC}"/* "${ZED_EXT_DIR}/"

echo ""
echo "=========================================="
echo "  ✅ Installation completed successfully!"
echo "=========================================="
echo ""
echo "Quick Start:"
echo "1. Verify installation in Zed:"
echo "   Press Cmd+Shift+P -> search 'zed: extensions' -> find 'Java Launcher'."
echo ""
echo "2. Initialize your Java project:"
echo "   cd /path/to/your/java/project"
echo "   ${INSTALL_BIN_DIR}/java-launcher sync-zed --write"
echo ""
echo "3. Launch in Zed:"
echo "   Press Cmd+Shift+P -> 'task: spawn' -> select '[Java Launcher] Open'"
echo "=========================================="
