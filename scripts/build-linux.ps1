#Requires -Version 5.1
[CmdletBinding()]
param(
    [string]$BinaryName = "session-web-linux-x86_64",
    [string]$ImageTag = "session-web-build"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$containerName = "session-web-tmp-$PID"
$containerCreated = $false

Push-Location -LiteralPath $root
try {
    Write-Host ">>> 构建 Linux musl Docker 镜像..." -ForegroundColor Cyan
    & docker build -t $ImageTag .
    if ($LASTEXITCODE -ne 0) {
        throw "Docker 镜像构建失败"
    }

    Write-Host ">>> 提取静态二进制..." -ForegroundColor Cyan
    & docker create --name $containerName $ImageTag | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "Docker 临时容器创建失败"
    }
    $containerCreated = $true

    & docker cp ($containerName + ":/usr/local/bin/session-web") "./$BinaryName"
    if ($LASTEXITCODE -ne 0) {
        throw "Docker 二进制提取失败"
    }
    if (-not (Test-Path -LiteralPath "./$BinaryName" -PathType Leaf)) {
        throw "提取完成后未找到文件: $BinaryName"
    }

    $size = (Get-Item -LiteralPath "./$BinaryName").Length / 1MB
    Write-Host (">>> 完成: ./{0} ({1:N1} MB)" -f $BinaryName, $size) -ForegroundColor Green
} finally {
    if ($containerCreated) {
        & docker rm -f $containerName | Out-Null
    }
    Pop-Location
}
