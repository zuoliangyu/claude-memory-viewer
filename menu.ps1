#Requires -Version 5.1
<#
.SYNOPSIS
AI Session Viewer 的统一开发、构建与维护菜单。

.PARAMETER Action
跳过交互菜单并直接执行指定动作，供兼容脚本和自动化使用。

.EXAMPLE
.\menu.ps1

.EXAMPLE
.\menu.ps1 -Action dev-perf
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
        "analyze-perf"
    )]
    [string]$Action = "menu"
)

$ErrorActionPreference = "Stop"
$script:MenuExitCode = 0

function Invoke-DesktopDev {
    param([switch]$PerfDiagnostics)

    $previousBackendDiagnostics = [Environment]::GetEnvironmentVariable(
        "ASV_PERF_DIAGNOSTICS",
        [EnvironmentVariableTarget]::Process
    )
    $previousFrontendDiagnostics = [Environment]::GetEnvironmentVariable(
        "VITE_ASV_PERF_DIAGNOSTICS",
        [EnvironmentVariableTarget]::Process
    )

    try {
        if ($PerfDiagnostics) {
            $perfDirectory = Join-Path $PSScriptRoot "target\perf"
            $perfLog = Join-Path $perfDirectory ("dev-{0}.log" -f (Get-Date -Format "yyyyMMdd-HHmmss"))
            New-Item -ItemType Directory -Force -Path $perfDirectory | Out-Null

            $env:ASV_PERF_DIAGNOSTICS = "1"
            $env:VITE_ASV_PERF_DIAGNOSTICS = "1"

            Write-Host "[ASV-PERF] 开发性能诊断已启用" -ForegroundColor Cyan
            Write-Host "[ASV-PERF] 日志文件: $perfLog"
            Write-Host "[ASV-PERF] 分析入口: .\menu.ps1 -> 分析性能日志"
            & npx tauri dev 2>&1 | Tee-Object -FilePath $perfLog
        } else {
            & npx tauri dev
        }
        $script:MenuExitCode = $LASTEXITCODE
    } finally {
        if ($PerfDiagnostics) {
            [Environment]::SetEnvironmentVariable(
                "ASV_PERF_DIAGNOSTICS",
                $previousBackendDiagnostics,
                [EnvironmentVariableTarget]::Process
            )
            [Environment]::SetEnvironmentVariable(
                "VITE_ASV_PERF_DIAGNOSTICS",
                $previousFrontendDiagnostics,
                [EnvironmentVariableTarget]::Process
            )
        }
    }
}

function Invoke-WebDev {
    & npm run build:web
    if ($LASTEXITCODE -ne 0) {
        $script:MenuExitCode = $LASTEXITCODE
        return
    }

    & cargo run -p session-web
    $script:MenuExitCode = $LASTEXITCODE
}

function Invoke-ChildPowerShell {
    param(
        [Parameter(Mandatory)]
        [string]$Path
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "本机脚本不存在: $Path"
    }

    $powerShellPath = (Get-Process -Id $PID).Path
    & $powerShellPath -NoProfile -ExecutionPolicy Bypass -File $Path
    $script:MenuExitCode = $LASTEXITCODE
}

function Invoke-DesktopBuild {
    $localBuildScript = Join-Path $PSScriptRoot "scripts\build.ps1"
    Invoke-ChildPowerShell -Path $localBuildScript
}

function Invoke-WebBuild {
    & npm run build:web
    if ($LASTEXITCODE -ne 0) {
        $script:MenuExitCode = $LASTEXITCODE
        return
    }

    & cargo build -p session-web --release
    $script:MenuExitCode = $LASTEXITCODE
    if ($script:MenuExitCode -eq 0) {
        Write-Host "构建完成: target/release/session-web.exe" -ForegroundColor Green
    }
}

function Invoke-ActionByName {
    param(
        [Parameter(Mandatory)]
        [string]$Name
    )

    $script:MenuExitCode = 0
    switch ($Name) {
        "dev" {
            Invoke-DesktopDev
        }
        "dev-perf" {
            Invoke-DesktopDev -PerfDiagnostics
        }
        "dev-web" {
            Invoke-WebDev
        }
        "build" {
            Invoke-DesktopBuild
        }
        "build-web" {
            Invoke-WebBuild
        }
        "analyze-perf" {
            Invoke-ChildPowerShell -Path (Join-Path $PSScriptRoot "scripts\analyze-perf-log.ps1")
        }
        default {
            throw "未知操作: $Name"
        }
    }
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
    Write-Host "统一开发与构建菜单"
    Write-Host
    Write-Host "开发" -ForegroundColor DarkCyan
    Write-Host "  1. 桌面应用开发"
    Write-Host "  2. 桌面应用开发（性能诊断日志）"
    Write-Host "  3. Web 服务器开发"
    Write-Host
    Write-Host "构建" -ForegroundColor DarkCyan
    Write-Host "  4. 桌面安装包（本地，不生成更新签名）"
    Write-Host "  5. Web 服务器（Windows）"
    Write-Host
    Write-Host "维护" -ForegroundColor DarkCyan
    Write-Host "  6. 分析性能日志"
    Write-Host
    Write-Host "  0. 退出"
    Write-Host
}

$originalLocation = (Get-Location).Path
try {
    Set-Location -LiteralPath $PSScriptRoot

    if ($Action -ne "menu") {
        Invoke-ActionByName -Name $Action
        exit $script:MenuExitCode
    }

    while ($true) {
        Show-Menu
        $choice = (Read-Host "请选择操作").Trim()
        switch ($choice) {
            "1" { Invoke-InteractiveAction -Name "dev" -Title "桌面应用开发" }
            "2" { Invoke-InteractiveAction -Name "dev-perf" -Title "桌面应用开发（性能诊断日志）" }
            "3" { Invoke-InteractiveAction -Name "dev-web" -Title "Web 服务器开发" }
            "4" { Invoke-InteractiveAction -Name "build" -Title "构建本地桌面安装包" }
            "5" { Invoke-InteractiveAction -Name "build-web" -Title "构建 Web 服务器" }
            "6" { Invoke-InteractiveAction -Name "analyze-perf" -Title "分析性能日志" }
            "0" { return }
            default {
                Write-Host "无效选项: $choice" -ForegroundColor Yellow
                Wait-ForMenu
            }
        }
    }
} finally {
    Set-Location -LiteralPath $originalLocation
}
