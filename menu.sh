#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
SCRIPTS_DIR="$ROOT_DIR/scripts"

run_action() {
  local name="$1"
  shift
  case "$name" in
    dev) "$SCRIPTS_DIR/dev.sh" "$@" ;;
    dev-perf) "$SCRIPTS_DIR/dev.sh" --perf "$@" ;;
    dev-web) "$SCRIPTS_DIR/dev-web.sh" "$@" ;;
    build) "$SCRIPTS_DIR/build.sh" "$@" ;;
    build-web) "$SCRIPTS_DIR/build-web.sh" "$@" ;;
    build-linux) "$SCRIPTS_DIR/build-linux.sh" "$@" ;;
    deploy-rocky) "$SCRIPTS_DIR/deploy-rocky.sh" "$@" ;;
    clean) "$SCRIPTS_DIR/clean.sh" "$@" ;;
    analyze-perf) "$SCRIPTS_DIR/analyze-perf-log.sh" "$@" ;;
    check) "$SCRIPTS_DIR/check.sh" "$@" ;;
    *)
      echo "未知操作: $name" >&2
      return 2
      ;;
  esac
}

wait_for_menu() {
  echo
  read -r -p "按 Enter 返回菜单" _
}

confirm_action() {
  local answer
  read -r -p "$1 [y/N] " answer
  [[ "$answer" == "y" || "$answer" == "Y" || "$answer" == "yes" ]]
}

run_interactive() {
  local name="$1"
  local title="$2"
  local exit_code
  echo
  echo ">>> $title"
  set +e
  run_action "$name"
  exit_code=$?
  set -e
  if ((exit_code == 0)); then
    echo
    echo "操作已结束。"
  else
    echo
    echo "操作失败，退出码: $exit_code" >&2
  fi
  wait_for_menu
}

show_menu() {
  if [[ -t 1 ]] && command -v clear >/dev/null 2>&1; then
    clear
  fi
  cat <<'MENU'
AI Session Viewer
Linux 开发与构建菜单

开发
  1. 桌面应用开发
  2. 桌面应用开发（性能诊断日志）
  3. Web 服务器开发

构建
  4. 桌面安装包（本地，不生成更新签名）
  5. Web 服务器
  6. Linux 静态文件（Docker）

维护
  7. 部署到 Rocky Linux
  8. 清理构建产物
  9. 分析性能日志
 10. 运行轻量检查

  0. 退出
MENU
  echo
}

action="${1:-menu}"
if [[ "$action" != "menu" ]]; then
  shift
  run_action "$action" "$@"
  exit $?
fi

while true; do
  show_menu
  read -r -p "请选择操作: " choice
  case "$choice" in
    1) run_interactive dev "桌面应用开发" ;;
    2) run_interactive dev-perf "桌面应用开发（性能诊断日志）" ;;
    3) run_interactive dev-web "Web 服务器开发" ;;
    4) run_interactive build "构建本地桌面安装包" ;;
    5) run_interactive build-web "构建 Web 服务器" ;;
    6) run_interactive build-linux "构建 Linux 静态文件" ;;
    7)
      if confirm_action "确认部署到 Rocky Linux？"; then
        run_interactive deploy-rocky "部署到 Rocky Linux"
      fi
      ;;
    8)
      if confirm_action "确认清理构建产物？"; then
        run_interactive clean "清理构建产物"
      fi
      ;;
    9) run_interactive analyze-perf "分析性能日志" ;;
    10) run_interactive check "运行轻量检查" ;;
    0) exit 0 ;;
    *)
      echo "无效选项: $choice" >&2
      wait_for_menu
      ;;
  esac
done
