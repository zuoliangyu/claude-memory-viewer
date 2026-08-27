#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"

[[ -f "$ROOT_DIR/package.json" && -d "$ROOT_DIR/src-tauri" ]] || {
  echo "无法确认仓库根目录: $ROOT_DIR" >&2
  exit 1
}

remove_deps=0
remove_all=0
show_stats=0
while (($# > 0)); do
  case "$1" in
    --deps) remove_deps=1 ;;
    --all) remove_all=1 ;;
    --stats) show_stats=1 ;;
    *)
      echo "未知参数: $1" >&2
      exit 2
      ;;
  esac
  shift
done

targets=(
  "dist"
  "target"
  "src-tauri/target"
  "src-tauri/gen"
  "tsconfig.tsbuildinfo"
  "node_modules/.vite"
)
if ((remove_deps || remove_all)); then
  targets+=("node_modules")
fi
if ((remove_all)); then
  targets+=("src-tauri/WixTools")
fi

removed=0
freed_kb=0
for relative in "${targets[@]}"; do
  full="$ROOT_DIR/$relative"
  case "$full" in
    "$ROOT_DIR"/*) ;;
    *)
      echo "拒绝清理仓库外路径: $full" >&2
      exit 1
      ;;
  esac
  if [[ ! -e "$full" ]]; then
    echo "  skip    $relative"
    continue
  fi

  size_kb=0
  if ((show_stats)); then
    size_kb="$(du -sk -- "$full" 2>/dev/null | awk '{print $1}')"
    size_kb="${size_kb:-0}"
  fi
  rm -rf -- "$full"
  removed=$((removed + 1))
  freed_kb=$((freed_kb + size_kb))
  if ((show_stats)); then
    echo "  removed $relative (${size_kb} KiB)"
  else
    echo "  removed $relative"
  fi
done

if ((show_stats)); then
  echo "完成：删除 $removed 项，释放约 ${freed_kb} KiB。"
else
  echo "完成：删除 $removed 项。"
fi
