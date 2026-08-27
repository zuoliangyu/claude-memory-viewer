# 脚本说明

根目录提供两个统一入口：

- Windows：`.\menu.ps1`
- Linux/macOS：`./menu.sh`

两个菜单的动作保持一致：

| 动作 | PowerShell | Bash |
|---|---|---|
| 桌面开发 | `scripts/dev.ps1` | `scripts/dev.sh` |
| 桌面开发（性能诊断） | `scripts/dev.ps1 -PerfDiagnostics` | `scripts/dev.sh --perf` |
| Web 开发 | `scripts/dev-web.ps1` | `scripts/dev-web.sh` |
| 本地桌面构建 | `scripts/build.ps1` | `scripts/build.sh` |
| Web 构建 | `scripts/build-web.ps1` | `scripts/build-web.sh` |
| Linux 静态构建 | `scripts/build-linux.ps1` | `scripts/build-linux.sh` |
| Rocky 部署 | `scripts/deploy-rocky.ps1` | `scripts/deploy-rocky.sh` |
| 清理 | `scripts/clean.ps1` | `scripts/clean.sh` |
| 性能日志分析 | `scripts/analyze-perf-log.ps1` | `scripts/analyze-perf-log.sh` |
| 轻量检查 | `scripts/check.ps1` | `scripts/check.sh` |

## 关键行为

- 普通开发默认不记录性能日志，必须显式选择性能诊断模式。
- 本地桌面构建临时设置 `createUpdaterArtifacts=false`，不需要发布签名私钥。
- 正式发布继续使用 `tauri.conf.json` 和 GitHub Actions Secrets 生成 updater 签名。
- 所有脚本都会先定位仓库根目录，因此可以从任意当前目录调用。

## 常用参数

```bash
# Web 服务参数会传给 session-web
./scripts/dev-web.sh --port 8080

# Rocky 部署，也可使用 ASV_DEPLOY_HOST / USER / PATH / FILE
./scripts/deploy-rocky.sh --host 192.168.124.133 --user root

# 清理依赖或显示释放空间
./scripts/clean.sh --deps --stats
./scripts/clean.sh --all --stats

# 指定性能日志和阈值
./scripts/analyze-perf-log.sh --path target/perf/dev-example.log --ipc-threshold-ms 5000
```
