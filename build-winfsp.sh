#!/usr/bin/env bash
set -euo pipefail

# Load settings
. "$(dirname "$0")/build-settings.sh"

# Reset the human-readable build log for this top-level run
mkdir -p assets
: > assets/build-report.txt   # truncate (or create)

echo "==> Ensuring target '${TARGET_TRIPLE}' is installed"
rustup target add "${TARGET_TRIPLE}" >/dev/null

# Clean previous bin artifacts to avoid stale bundles
if [ -d "${BIN_DIR}" ]; then
  echo "==> Removing existing ${BIN_DIR}/"
  rm -rf "${BIN_DIR}"
fi

# Prepare bin/
echo "==> Ensuring ./${BIN_DIR} exists"
mkdir -p "${BIN_DIR}"

# 1) Build viewer executable (mpq-viewer.exe)
echo "==> Building viewer executable: target=${TARGET_TRIPLE}, profile=${PROFILE}"
cargo build --target "${TARGET_TRIPLE}" --${PROFILE} --bin mpq-viewer

VIEWER_PATH="target/${TARGET_TRIPLE}/${PROFILE}/mpq-viewer.exe"

# Check viewer EXE
[ -f "${VIEWER_PATH}" ] || { echo "ERR: Viewer EXE not found: ${VIEWER_PATH}"; exit 1; }

# Copy viewer into ./bin so installer can include_bytes! it at compile-time
cp -f "${VIEWER_PATH}" "${BIN_DIR}/"
echo "==> Copied viewer to ${BIN_DIR}/$(basename "${VIEWER_PATH}")"

# 2) Build installer (icon embedding happens only now)
echo "==> Building installer (embeds ./bin/mpq-viewer.exe via include_bytes!)"

# Force build.rs to re-run for the installer phase
printf "phase=installer ts=%s\n" "$(date +%s%N)" >> assets/build-report.txt

# Set env only for the installer build so build.rs knows to embed resources
MPQ_INSTALLER=1 cargo build --target "${TARGET_TRIPLE}" --${PROFILE} --bin "${BIN_NAME}"

# Check installer EXE
[ -f "${EXE_PATH}" ] || { echo "ERR: Installer EXE not found: ${EXE_PATH}"; exit 1; }

# Copy installer into ./bin
cp -f "${EXE_PATH}" "${BIN_DIR}/"
echo "==> Copied installer to ${BIN_DIR}/$(basename "${EXE_PATH}")"

echo "==> Done. Artifacts in ${BIN_DIR}/:"
ls -lh "${BIN_DIR}"
