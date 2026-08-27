#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT_DIR"

perf_diagnostics=0
if [[ "${1:-}" == "--perf" ]]; then
  perf_diagnostics=1
  shift
fi

if ((perf_diagnostics)); then
  perf_directory="$ROOT_DIR/target/perf"
  perf_log="$perf_directory/dev-$(date +%Y%m%d-%H%M%S).log"
  mkdir -p -- "$perf_directory"
  echo "[ASV-PERF] 性能诊断已启用"
  echo "[ASV-PERF] 日志文件: $perf_log"
  ASV_PERF_DIAGNOSTICS=1 VITE_ASV_PERF_DIAGNOSTICS=1 \
    npx tauri dev "$@" 2>&1 | tee "$perf_log"
else
  exec npx tauri dev "$@"
fi
