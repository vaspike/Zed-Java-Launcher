#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist"
STAGE_DIR="${DIST_DIR}/zed-java-launcher"
VERSION="0.1.0"
ARCH="$(uname -m)"
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
PACKAGE_NAME="zed-java-launcher-v${VERSION}-${OS}-${ARCH}"

echo "=========================================="
echo "  Building and Packaging Zed Java Launcher"
echo "  Version: ${VERSION}, Platform: ${OS}-${ARCH}"
echo "=========================================="

# 1. Build release binary
echo "-> [1/4] Compiling release binary with cargo..."
cd "${ROOT_DIR}"
cargo build --release

BIN_PATH="${ROOT_DIR}/target/release/java-launcher"
if [[ ! -f "${BIN_PATH}" ]]; then
    echo "❌ Error: Binary not found at ${BIN_PATH}" >&2
    exit 1
fi

# 2. Prepare staging directory
echo "-> [2/4] Setting up distribution structure at ${STAGE_DIR}..."
rm -rf "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}/bin"
mkdir -p "${STAGE_DIR}/extension"

# Copy binary
cp -f "${BIN_PATH}" "${STAGE_DIR}/bin/java-launcher"
chmod +x "${STAGE_DIR}/bin/java-launcher"

# Copy extension files
cp -f "${ROOT_DIR}/extension/extension.toml" "${STAGE_DIR}/extension/extension.toml"
cp -f "${ROOT_DIR}/extension/README.md" "${STAGE_DIR}/extension/README.md"

# Copy scripts & doc
cp -f "${ROOT_DIR}/scripts/install.sh" "${STAGE_DIR}/install.sh"
cp -f "${ROOT_DIR}/scripts/uninstall.sh" "${STAGE_DIR}/uninstall.sh"
cp -f "${ROOT_DIR}/scripts/dist-README.md" "${STAGE_DIR}/README.md"
chmod +x "${STAGE_DIR}/install.sh" "${STAGE_DIR}/uninstall.sh"

# 3. Create compressed archives
echo "-> [3/4] Creating distribution archives..."
TAR_FILE="${DIST_DIR}/${PACKAGE_NAME}.tar.gz"
ZIP_FILE="${DIST_DIR}/${PACKAGE_NAME}.zip"

cd "${DIST_DIR}"
rm -f "${TAR_FILE}" "${ZIP_FILE}"

tar -czf "${TAR_FILE}" zed-java-launcher
zip -r -q "${ZIP_FILE}" zed-java-launcher

# 4. Summary
echo "-> [4/4] Package built successfully!"
echo "=========================================="
echo "Generated distribution artifacts:"
ls -lh "${DIST_DIR}/${PACKAGE_NAME}".*
echo ""
echo "Unpacked directory ready for inspection:"
echo "  ${STAGE_DIR}"
echo "=========================================="
