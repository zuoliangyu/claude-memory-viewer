#Requires -Version 5.1

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot

Push-Location -LiteralPath $root
try {
    & npm run check:scripts
    exit $LASTEXITCODE
} finally {
    Pop-Location
}
