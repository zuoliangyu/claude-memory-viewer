[CmdletBinding()]
param(
    [switch]$PerfDiagnostics,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$TauriArguments = @()
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$exitCode = 1
$previousBackendDiagnostics = [Environment]::GetEnvironmentVariable(
    "ASV_PERF_DIAGNOSTICS",
    [EnvironmentVariableTarget]::Process
)
$previousFrontendDiagnostics = [Environment]::GetEnvironmentVariable(
    "VITE_ASV_PERF_DIAGNOSTICS",
    [EnvironmentVariableTarget]::Process
)

Push-Location -LiteralPath $root
try {
    if ($PerfDiagnostics) {
        $perfDirectory = Join-Path $root "target\perf"
        $perfLog = Join-Path $perfDirectory ("dev-{0}.log" -f (Get-Date -Format "yyyyMMdd-HHmmss"))
        New-Item -ItemType Directory -Force -Path $perfDirectory | Out-Null
        $env:ASV_PERF_DIAGNOSTICS = "1"
        $env:VITE_ASV_PERF_DIAGNOSTICS = "1"

        Write-Host "[ASV-PERF] 性能诊断已启用" -ForegroundColor Cyan
        Write-Host "[ASV-PERF] 日志文件: $perfLog"
        & npx tauri dev @TauriArguments 2>&1 | Tee-Object -FilePath $perfLog
    } else {
        & npx tauri dev @TauriArguments
    }
    $exitCode = $LASTEXITCODE
} finally {
    [Environment]::SetEnvironmentVariable(
        "ASV_PERF_DIAGNOSTICS",
        $previousBackendDiagnostics,
        [EnvironmentVariableTarget]::Process
    )
    [Environment]::SetEnvironmentVariable(
        "VITE_ASV_PERF_DIAGNOSTICS",
        $previousFrontendDiagnostics,
        [EnvironmentVariableTarget]::Process
    )
    Pop-Location
}

exit $exitCode
