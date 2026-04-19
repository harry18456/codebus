#!/usr/bin/env bash
# start-qdrant.sh — launch a Qdrant standalone binary locally.
# Backs openspec/changes/m1-power-on/specs/qdrant-client/spec.md
#   Requirement: Local Qdrant launch recipe
#
# Resolves the binary via, in order:
#   1. $CODEBUS_QDRANT_BIN (absolute path to the qdrant binary)
#   2. ~/.codebus/bin/qdrant
# Persistent storage defaults to ~/.codebus/kb/, overridable via
# $CODEBUS_QDRANT_STORAGE.  See docs/decisions.md D-027.
set -euo pipefail

DOWNLOAD_URL="https://github.com/qdrant/qdrant/releases"
DEFAULT_BIN="${HOME}/.codebus/bin/qdrant"
DEFAULT_STORAGE="${HOME}/.codebus/kb"

bin_path="${CODEBUS_QDRANT_BIN:-${DEFAULT_BIN}}"
storage_path="${CODEBUS_QDRANT_STORAGE:-${DEFAULT_STORAGE}}"
snapshots_path="${storage_path}/snapshots"

if [[ ! -x "${bin_path}" ]]; then
  cat >&2 <<EOF
[start-qdrant] Qdrant binary not found at: ${bin_path}

Download the standalone binary for your platform from:
  ${DOWNLOAD_URL}

Then drop the extracted file at:
  ${DEFAULT_BIN}
or set CODEBUS_QDRANT_BIN to its absolute path.
EOF
  exit 1
fi

mkdir -p "${storage_path}" "${snapshots_path}"

# Qdrant configures storage via env vars, not CLI flags (as of v1.17).
# Set both storage + snapshots paths so nothing pollutes $CWD.
export QDRANT__STORAGE__STORAGE_PATH="${storage_path}"
export QDRANT__STORAGE__SNAPSHOTS_PATH="${snapshots_path}"
exec "${bin_path}" "$@"
