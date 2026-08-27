[CmdletBinding()]
param(
    [switch]$PerfDiagnostics
)

$action = if ($PerfDiagnostics) { "dev-perf" } else { "dev" }
$menuPath = Join-Path $PSScriptRoot "..\menu.ps1"
& $menuPath -Action $action
exit $LASTEXITCODE
