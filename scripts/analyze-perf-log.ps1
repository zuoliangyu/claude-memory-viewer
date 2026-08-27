param(
    [string]$Path,
    [double]$IpcThresholdMs = 5000
)

$ErrorActionPreference = "Stop"

if (-not $Path) {
    $latest = Get-ChildItem (Join-Path $PSScriptRoot "..\target\perf\dev-*.log") -File |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1
    if (-not $latest) {
        throw "未找到 target\perf\dev-*.log，请先通过 .\menu.ps1 选择「桌面应用开发（性能诊断日志）」复现问题。"
    }
    $Path = $latest.FullName
}

$events = foreach ($line in Get-Content -LiteralPath $Path) {
    if ($line -notmatch '^\[ASV-PERF\]\s+(\{.*\})$') { continue }
    try { $Matches[1] | ConvertFrom-Json } catch { continue }
}

$ipcEvents = @($events | Where-Object name -eq "messages.ipc_roundtrip")
if ($ipcEvents.Count -eq 0) {
    throw "日志中没有 messages.ipc_roundtrip，尚未完成一次消息读取。"
}

$slowestIpc = $ipcEvents |
    Sort-Object { [double]$_.durationMs } -Descending |
    Select-Object -First 1
$slowestRefresh = $events |
    Where-Object name -eq "background_refresh.completed" |
    Sort-Object { [double]$_.durationMs } -Descending |
    Select-Object -First 1
$slowestSessionCost = $events |
    Where-Object name -eq "stats.session_cost_backend" |
    Sort-Object { [double]$_.durationMs } -Descending |
    Select-Object -First 1
$longTasks = @($events | Where-Object name -eq "browser.long_task")

Write-Host "日志: $Path"
Write-Host ("最慢消息 IPC: {0:N1} ms（{1} 条消息，约 {2} MB 文本）" -f `
    [double]$slowestIpc.durationMs, $slowestIpc.fields.messages, $slowestIpc.fields.approximateTextMb)
if ($slowestRefresh) {
    Write-Host ("最慢后台刷新: {0:N1} ms（reason={1}, forceReload={2}）" -f `
        [double]$slowestRefresh.durationMs, $slowestRefresh.fields.reason, $slowestRefresh.fields.forceReload)
}
if ($slowestSessionCost) {
    Write-Host ("最慢会话账单: {0:N1} ms（{1} 次请求）" -f `
        [double]$slowestSessionCost.durationMs, $slowestSessionCost.fields.requests)
}
Write-Host ("浏览器长任务: {0}" -f $longTasks.Count)

if ([double]$slowestIpc.durationMs -gt $IpcThresholdMs) {
    Write-Error ("FAIL: 消息 IPC 超过 {0:N0} ms 阈值" -f $IpcThresholdMs)
}

Write-Host ("PASS: 消息 IPC 未超过 {0:N0} ms 阈值" -f $IpcThresholdMs)
