npm run build:web
cargo build -p session-web --release @args
Write-Host "`nBuild complete: target/release/session-web.exe" -ForegroundColor Green
