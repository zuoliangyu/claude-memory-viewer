#Requires -Version 5.1

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$exitCode = 1

Push-Location -LiteralPath $root
try {
    & npm run build:web
    if ($LASTEXITCODE -ne 0) {
        $exitCode = $LASTEXITCODE
    } else {
        & cargo run -p session-web -- @args
        $exitCode = $LASTEXITCODE
    }
} finally {
    Pop-Location
}

exit $exitCode
