# 构建 Rocky Linux 9.4 可运行的 musl 静态单文件
# 输出：session-web-linux-x86_64

$OutputEncoding = [System.Text.Encoding]::UTF8
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$ErrorActionPreference = "Stop"
$BinaryName = "session-web-linux-x86_64"
$ImageTag = "session-web-build"
$ContainerName = "session-web-tmp"

Write-Host ">>> 构建 Docker 镜像（musl 静态链接）..." -ForegroundColor Cyan
docker build -t $ImageTag .
if ($LASTEXITCODE -ne 0) { Write-Error "Docker build 失败"; exit 1 }

Write-Host ">>> 提取二进制文件..." -ForegroundColor Cyan
docker create --name $ContainerName $ImageTag | Out-Null
docker cp "${ContainerName}:/usr/local/bin/session-web" "./$BinaryName"
docker rm $ContainerName | Out-Null

if (Test-Path "./$BinaryName") {
    $size = (Get-Item "./$BinaryName").Length / 1MB
    Write-Host ">>> 完成！文件：./$BinaryName ($("{0:N1}" -f $size) MB)" -ForegroundColor Green
} else {
    Write-Error "提取失败，未找到文件"
    exit 1
}
