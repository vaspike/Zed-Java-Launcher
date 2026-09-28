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
echo "  Building Standard Zed Extension Package"
echo "  Version: ${VERSION}, Platform: ${OS}-${ARCH}"
echo "=========================================="

# 1. Build native CLI binary
echo "-> [1/4] Compiling native CLI binary with cargo..."
cd "${ROOT_DIR}"
cargo build --release

BIN_PATH="${ROOT_DIR}/target/release/java-launcher"
if [[ ! -f "${BIN_PATH}" ]]; then
    echo "❌ Error: Binary not found at ${BIN_PATH}" >&2
    exit 1
fi

# 2. Build Zed WASM extension
echo "-> [2/4] Compiling Zed WASM extension (wasm32-wasip2)..."
cargo build --manifest-path "${ROOT_DIR}/zed-extension/Cargo.toml" --target wasm32-wasip2 --release

WASM_PATH="${ROOT_DIR}/zed-extension/target/wasm32-wasip2/release/zed_java_launcher.wasm"
if [[ ! -f "${WASM_PATH}" ]]; then
    echo "❌ Error: WASM binary not found at ${WASM_PATH}" >&2
    exit 1
fi

# 3. Prepare standard Zed extension directory
echo "-> [3/4] Staging standard Zed extension at ${STAGE_DIR}..."
rm -rf "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}/bin"
mkdir -p "${STAGE_DIR}/src"

# Copy standard extension files to ROOT of package
cp -f "${ROOT_DIR}/extension.toml" "${STAGE_DIR}/extension.toml"
cp -f "${WASM_PATH}" "${STAGE_DIR}/extension.wasm"
cp -f "${ROOT_DIR}/zed-extension/Cargo.toml" "${STAGE_DIR}/Cargo.toml"
cp -f "${ROOT_DIR}/zed-extension/src/lib.rs" "${STAGE_DIR}/src/lib.rs"
cp -r "${ROOT_DIR}/debug_adapter_schemas" "${STAGE_DIR}/debug_adapter_schemas"

# Copy native CLI binary
cp -f "${BIN_PATH}" "${STAGE_DIR}/bin/java-launcher"
chmod +x "${STAGE_DIR}/bin/java-launcher"

# Copy scripts & doc
cp -f "${ROOT_DIR}/scripts/install.sh" "${STAGE_DIR}/install.sh"
cp -f "${ROOT_DIR}/scripts/uninstall.sh" "${STAGE_DIR}/uninstall.sh"
cp -f "${ROOT_DIR}/scripts/dist-README.md" "${STAGE_DIR}/README.md"
chmod +x "${STAGE_DIR}/install.sh" "${STAGE_DIR}/uninstall.sh"

# 4. Create compressed archives
echo "-> [4/4] Creating distribution archives..."
TAR_FILE="${DIST_DIR}/${PACKAGE_NAME}.tar.gz"
ZIP_FILE="${DIST_DIR}/${PACKAGE_NAME}.zip"

cd "${DIST_DIR}"
rm -f "${TAR_FILE}" "${ZIP_FILE}"

tar -czf "${TAR_FILE}" zed-java-launcher
zip -r -q "${ZIP_FILE}" zed-java-launcher

echo ""
echo "=========================================="
echo "  ✅ Standard package built successfully!"
echo "=========================================="
echo "Distribution files:"
ls -lh "${DIST_DIR}/${PACKAGE_NAME}".*
echo ""
echo "Unpacked directory for Zed 'Install Dev Extension':"
echo "  ${STAGE_DIR}"
echo "=========================================="
