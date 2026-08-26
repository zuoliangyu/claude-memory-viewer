$ErrorActionPreference = "Stop"

$perfDirectory = Join-Path $PSScriptRoot "target\perf"
$perfLog = Join-Path $perfDirectory ("dev-{0}.log" -f (Get-Date -Format "yyyyMMdd-HHmmss"))
New-Item -ItemType Directory -Force -Path $perfDirectory | Out-Null

$env:ASV_PERF_DIAGNOSTICS = "1"
$env:VITE_ASV_PERF_DIAGNOSTICS = "1"
$devExitCode = 1

Write-Host "[ASV-PERF] 开发性能诊断已启用"
Write-Host "[ASV-PERF] 日志文件: $perfLog"

try {
    & npx tauri dev 2>&1 | Tee-Object -FilePath $perfLog
    $devExitCode = $LASTEXITCODE
} finally {
    Remove-Item Env:ASV_PERF_DIAGNOSTICS -ErrorAction SilentlyContinue
    Remove-Item Env:VITE_ASV_PERF_DIAGNOSTICS -ErrorAction SilentlyContinue
}

exit $devExitCode
