#Requires -Version 5.1

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$exitCode = 1

# Local packages are not release artifacts. Disable updater artifact generation
# for this invocation so no release signing key or password is required.
$localBuildConfig = '{"bundle":{"createUpdaterArtifacts":false}}'

Push-Location -LiteralPath $root
try {
    & npx tauri build --config $localBuildConfig @args
    $exitCode = $LASTEXITCODE
} finally {
    Pop-Location
}

exit $exitCode
