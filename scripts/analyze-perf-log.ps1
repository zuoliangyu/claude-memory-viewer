#Requires -Version 5.1
[CmdletBinding()]
param(
    [string]$Path,
    [double]$IpcThresholdMs = 5000
)

$ErrorActionPreference = "Stop"
$arguments = @(
    (Join-Path $PSScriptRoot "analyze-perf-log.mjs"),
    "--ipc-threshold-ms",
    $IpcThresholdMs
)
if ($Path) {
    $arguments += @("--path", $Path)
}

& node @arguments
exit $LASTEXITCODE
