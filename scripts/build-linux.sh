#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT_DIR"

binary_name="${ASV_LINUX_BINARY_NAME:-session-web-linux-x86_64}"
image_tag="${ASV_LINUX_IMAGE_TAG:-session-web-build}"
container_name="session-web-tmp-$$"
container_created=0

cleanup() {
  if ((container_created)); then
    docker rm -f "$container_name" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT INT TERM

echo ">>> 构建 Linux musl Docker 镜像..."
docker build -t "$image_tag" .

echo ">>> 提取静态二进制..."
docker create --name "$container_name" "$image_tag" >/dev/null
container_created=1
docker cp "$container_name:/usr/local/bin/session-web" "./$binary_name"

if [[ ! -f "$binary_name" ]]; then
  echo "提取完成后未找到文件: $binary_name" >&2
  exit 1
fi

size_bytes="$(wc -c <"$binary_name" | tr -d ' ')"
echo ">>> 完成: ./$binary_name ($size_bytes bytes)"
