# 上传 session-web 到 Rocky 9.4 并设置权限
# 默认目标：root@192.168.124.133

param(
    [string]$RemoteHost = "192.168.124.133",
    [string]$User = "root",
    [string]$RemotePath = "/home/zuolan/Desktop/session-web",
    [string]$LocalFile = "./session-web-linux-x86_64"
)

$ErrorActionPreference = "Stop"
$Target = "${User}@${RemoteHost}"

if (-not (Test-Path $LocalFile)) {
    Write-Error "本地文件不存在：$LocalFile，请先运行 .\scripts\build-linux.ps1"
    exit 1
}

Write-Host ">>> 上传到 ${Target}:${RemotePath} ..." -ForegroundColor Cyan
scp $LocalFile "${Target}:${RemotePath}"
if ($LASTEXITCODE -ne 0) { Write-Error "scp 上传失败"; exit 1 }

Write-Host ">>> 设置执行权限 + SELinux 标记..." -ForegroundColor Cyan
ssh $Target "chmod +x $RemotePath && chcon -t bin_t $RemotePath"
if ($LASTEXITCODE -ne 0) { Write-Error "chmod/chcon 失败"; exit 1 }

Write-Host ">>> 部署完成！" -ForegroundColor Green
Write-Host "    登录后运行：$RemotePath --port 3000" -ForegroundColor Gray
Write-Host ('    或后台运行：nohup ' + $RemotePath + ' --port 3000 > /tmp/session-web.log 2>&1 &') -ForegroundColor Gray
