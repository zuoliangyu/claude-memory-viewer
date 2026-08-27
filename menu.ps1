#Requires -Version 5.1
<#
.SYNOPSIS
AI Session Viewer 的 Windows 开发、构建与维护菜单。

.PARAMETER Action
跳过交互菜单并直接执行指定动作。
#>
[CmdletBinding()]
param(
    [ValidateSet(
        "menu",
        "dev",
        "dev-perf",
        "dev-web",
        "build",
        "build-web",
        "build-linux",
        "deploy-rocky",
        "clean",
        "analyze-perf",
        "check"
    )]
    [string]$Action = "menu",
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$ActionArguments = @()
)

$ErrorActionPreference = "Stop"
$script:MenuExitCode = 0
$scriptsDirectory = Join-Path $PSScriptRoot "scripts"

function Invoke-ChildPowerShell {
    param(
        [Parameter(Mandatory)]
        [string]$Name,
        [string[]]$Arguments = @()
    )

    $path = Join-Path $scriptsDirectory $Name
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "脚本不存在: $path"
    }

    $powerShellPath = (Get-Process -Id $PID).Path
    & $powerShellPath -NoProfile -ExecutionPolicy Bypass -File $path @Arguments
    $script:MenuExitCode = $LASTEXITCODE
}

function Invoke-ActionByName {
    param(
        [Parameter(Mandatory)]
        [string]$Name,
        [string[]]$Arguments = @()
    )

    $script:MenuExitCode = 0
    switch ($Name) {
        "dev" { Invoke-ChildPowerShell -Name "dev.ps1" -Arguments $Arguments }
        "dev-perf" {
            Invoke-ChildPowerShell -Name "dev.ps1" -Arguments (@("-PerfDiagnostics") + $Arguments)
        }
        "dev-web" { Invoke-ChildPowerShell -Name "dev-web.ps1" -Arguments $Arguments }
        "build" { Invoke-ChildPowerShell -Name "build.ps1" -Arguments $Arguments }
        "build-web" { Invoke-ChildPowerShell -Name "build-web.ps1" -Arguments $Arguments }
        "build-linux" { Invoke-ChildPowerShell -Name "build-linux.ps1" -Arguments $Arguments }
        "deploy-rocky" { Invoke-ChildPowerShell -Name "deploy-rocky.ps1" -Arguments $Arguments }
        "clean" { Invoke-ChildPowerShell -Name "clean.ps1" -Arguments $Arguments }
        "analyze-perf" {
            Invoke-ChildPowerShell -Name "analyze-perf-log.ps1" -Arguments $Arguments
        }
        "check" { Invoke-ChildPowerShell -Name "check.ps1" -Arguments $Arguments }
        default { throw "未知操作: $Name" }
    }
}

function Confirm-Action {
    param([string]$Prompt)

    $answer = (Read-Host "$Prompt [y/N]").Trim().ToLowerInvariant()
    return $answer -in @("y", "yes", "是")
}

function Wait-ForMenu {
    Write-Host
    [void](Read-Host "按 Enter 返回菜单")
}

function Invoke-InteractiveAction {
    param(
        [string]$Name,
        [string]$Title
    )

    Write-Host
    Write-Host ">>> $Title" -ForegroundColor Cyan
    try {
        Invoke-ActionByName -Name $Name
        Write-Host
        if ($script:MenuExitCode -eq 0) {
            Write-Host "操作已结束。" -ForegroundColor Green
        } else {
            Write-Host "操作失败，退出码: $script:MenuExitCode" -ForegroundColor Red
        }
    } catch {
        $script:MenuExitCode = 1
        Write-Host
        Write-Host "操作失败: $($_.Exception.Message)" -ForegroundColor Red
    }
    Wait-ForMenu
}

function Show-Menu {
    Clear-Host
    Write-Host "AI Session Viewer" -ForegroundColor Cyan
    Write-Host "Windows 开发与构建菜单"
    Write-Host
    Write-Host "开发" -ForegroundColor DarkCyan
    Write-Host "  1. 桌面应用开发"
    Write-Host "  2. 桌面应用开发（性能诊断日志）"
    Write-Host "  3. Web 服务器开发"
    Write-Host
    Write-Host "构建" -ForegroundColor DarkCyan
    Write-Host "  4. 桌面安装包（本地，不生成更新签名）"
    Write-Host "  5. Web 服务器"
    Write-Host "  6. Linux 静态文件（Docker）"
    Write-Host
    Write-Host "维护" -ForegroundColor DarkCyan
    Write-Host "  7. 部署到 Rocky Linux"
    Write-Host "  8. 清理构建产物"
    Write-Host "  9. 分析性能日志"
    Write-Host " 10. 运行轻量检查"
    Write-Host
    Write-Host "  0. 退出"
    Write-Host
}

if ($Action -ne "menu") {
    Invoke-ActionByName -Name $Action -Arguments $ActionArguments
    exit $script:MenuExitCode
}

while ($true) {
    Show-Menu
    $choice = (Read-Host "请选择操作").Trim()
    switch ($choice) {
        "1" { Invoke-InteractiveAction -Name "dev" -Title "桌面应用开发" }
        "2" {
            Invoke-InteractiveAction -Name "dev-perf" -Title "桌面应用开发（性能诊断日志）"
        }
        "3" { Invoke-InteractiveAction -Name "dev-web" -Title "Web 服务器开发" }
        "4" { Invoke-InteractiveAction -Name "build" -Title "构建本地桌面安装包" }
        "5" { Invoke-InteractiveAction -Name "build-web" -Title "构建 Web 服务器" }
        "6" { Invoke-InteractiveAction -Name "build-linux" -Title "构建 Linux 静态文件" }
        "7" {
            if (Confirm-Action "确认部署到 Rocky Linux？") {
                Invoke-InteractiveAction -Name "deploy-rocky" -Title "部署到 Rocky Linux"
            }
        }
        "8" {
            if (Confirm-Action "确认清理构建产物？") {
                Invoke-InteractiveAction -Name "clean" -Title "清理构建产物"
            }
        }
        "9" { Invoke-InteractiveAction -Name "analyze-perf" -Title "分析性能日志" }
        "10" { Invoke-InteractiveAction -Name "check" -Title "运行轻量检查" }
        "0" { return }
        default {
            Write-Host "无效选项: $choice" -ForegroundColor Yellow
            Wait-ForMenu
        }
    }
}
