#Requires -Version 5.1

$ErrorActionPreference = "Stop"

# Local packages are not release artifacts. Disable updater artifact generation
# for this invocation so no release signing key or password is required.
$localBuildConfig = '{"bundle":{"createUpdaterArtifacts":false}}'

& npx tauri build --config $localBuildConfig @args
exit $LASTEXITCODE
