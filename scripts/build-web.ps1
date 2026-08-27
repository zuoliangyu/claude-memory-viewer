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
        & cargo build -p session-web --release @args
        $exitCode = $LASTEXITCODE
        if ($exitCode -eq 0) {
            $extension = if ([Environment]::OSVersion.Platform -eq "Win32NT") { ".exe" } else { "" }
            Write-Host
            Write-Host "构建完成: target/release/session-web$extension" -ForegroundColor Green
        }
    }
} finally {
    Pop-Location
}

exit $exitCode
