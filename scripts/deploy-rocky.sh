#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT_DIR"

remote_host="${ASV_DEPLOY_HOST:-192.168.124.133}"
remote_user="${ASV_DEPLOY_USER:-root}"
remote_path="${ASV_DEPLOY_PATH:-/home/zuolan/Desktop/session-web}"
local_file="${ASV_DEPLOY_FILE:-session-web-linux-x86_64}"

require_value() {
  if (($# < 2)) || [[ -z "$2" ]]; then
    echo "参数 $1 缺少值" >&2
    exit 2
  fi
}

while (($# > 0)); do
  case "$1" in
    --host)
      require_value "$@"
      remote_host="$2"
      shift 2
      ;;
    --user)
      require_value "$@"
      remote_user="$2"
      shift 2
      ;;
    --remote-path)
      require_value "$@"
      remote_path="$2"
      shift 2
      ;;
    --file)
      require_value "$@"
      local_file="$2"
      shift 2
      ;;
    *)
      echo "未知参数: $1" >&2
      exit 2
      ;;
  esac
done

[[ "$remote_host" =~ ^[A-Za-z0-9._:-]+$ ]] || {
  echo "remote host 包含不安全字符" >&2
  exit 2
}
[[ "$remote_user" =~ ^[A-Za-z0-9._-]+$ ]] || {
  echo "remote user 包含不安全字符" >&2
  exit 2
}
[[ "$remote_path" =~ ^/[A-Za-z0-9._/-]+$ ]] || {
  echo "remote path 必须是仅包含常规路径字符的绝对路径" >&2
  exit 2
}
[[ -f "$local_file" ]] || {
  echo "本地文件不存在: $local_file，请先运行 scripts/build-linux.sh" >&2
  exit 1
}

target="$remote_user@$remote_host"
echo ">>> 上传到 $target:$remote_path ..."
scp -- "$local_file" "$target:$remote_path"
ssh -- "$target" "chmod +x '$remote_path' && chcon -t bin_t '$remote_path'"
echo ">>> 部署完成: $remote_path"
