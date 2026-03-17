#!/usr/bin/env bash
# Build the hub server sidecar binary and place it in src-tauri/binaries/
# Tauri requires the binary to be named: schedula-hub-{target-triple}
#
# Usage:
#   ./scripts/build-hub-sidecar.sh           # build for current host target
#   ./scripts/build-hub-sidecar.sh --release # same, explicit release (default)

set -euo pipefail

TARGET=$(rustc -vV | grep '^host:' | awk '{print $2}')
echo "Building hub sidecar for target: ${TARGET}"

cargo build --release --manifest-path hub-server/Cargo.toml

mkdir -p src-tauri/binaries

if [[ "${OSTYPE:-}" == "msys"* || "${OSTYPE:-}" == "win32"* || "${OS:-}" == "Windows_NT" ]]; then
    SRC="hub-server/target/release/schedula-hub.exe"
    DST="src-tauri/binaries/schedula-hub-${TARGET}.exe"
else
    SRC="hub-server/target/release/schedula-hub"
    DST="src-tauri/binaries/schedula-hub-${TARGET}"
fi

cp "${SRC}" "${DST}"
echo "Sidecar ready: ${DST}"
