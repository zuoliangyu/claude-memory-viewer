#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT_DIR"

local_build_config='{"bundle":{"createUpdaterArtifacts":false}}'
exec npx tauri build --config "$local_build_config" "$@"
