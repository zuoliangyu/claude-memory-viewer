#Requires -Version 5.1
[CmdletBinding()]
param(
    [string]$RemoteHost = "192.168.124.133",
    [string]$User = "root",
    [string]$RemotePath = "/home/zuolan/Desktop/session-web",
    [string]$LocalFile = "session-web-linux-x86_64"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot

if ($RemoteHost -notmatch '^[A-Za-z0-9._:-]+$') {
    throw "RemoteHost 包含不安全字符"
}
if ($User -notmatch '^[A-Za-z0-9._-]+$') {
    throw "User 包含不安全字符"
}
if ($RemotePath -notmatch '^/[A-Za-z0-9._/-]+$') {
    throw "RemotePath 必须是仅包含常规路径字符的绝对路径"
}

Push-Location -LiteralPath $root
try {
    if (-not (Test-Path -LiteralPath $LocalFile -PathType Leaf)) {
        throw "本地文件不存在: $LocalFile，请先运行 scripts/build-linux.ps1"
    }

    $target = $User + "@" + $RemoteHost
    $uploadTarget = $target + ":" + $RemotePath
    Write-Host ">>> 上传到 $uploadTarget ..." -ForegroundColor Cyan
    & scp -- $LocalFile $uploadTarget
    if ($LASTEXITCODE -ne 0) {
        throw "scp 上传失败"
    }

    & ssh -- $target "chmod +x '$RemotePath' && chcon -t bin_t '$RemotePath'"
    if ($LASTEXITCODE -ne 0) {
        throw "远端 chmod/chcon 失败"
    }
    Write-Host ">>> 部署完成: $RemotePath" -ForegroundColor Green
} finally {
    Pop-Location
}
