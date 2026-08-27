#Requires -Version 5.1
<#
.SYNOPSIS
    One-shot cleanup of build artifacts for this Tauri + Vite + Cargo project.

.DESCRIPTION
    Removes generated output so the next build starts clean. By default it
    deletes build artifacts only (safe, fast to regenerate). Dependencies
    (node_modules) and the downloaded WiX installer toolchain are kept unless
    you opt in, because re-fetching them is slow.

.PARAMETER Deps
    Also remove node_modules/ (forces a fresh `npm install` next time).

.PARAMETER All
    Remove everything: build artifacts + node_modules + src-tauri/WixTools.

.PARAMETER Stats
    Measure and report the disk space freed (adds an extra scan pass).

.EXAMPLE
    .\scripts\clean.ps1
    Clean build artifacts only.

.EXAMPLE
    .\scripts\clean.ps1 -All -Stats
    Full reset and show how much was freed.
#>
[CmdletBinding()]
param(
    [switch]$Deps,
    [switch]$All,
    [switch]$Stats
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
if (-not (Test-Path -LiteralPath (Join-Path $root "package.json") -PathType Leaf)) {
    throw "无法确认仓库根目录: $root"
}

# Build artifacts cleaned by default. Order does not matter; each is removed
# independently and missing ones are skipped.
$targets = @(
    'dist',                  # Vite frontend output
    'target',                # Cargo workspace build output (root)
    'src-tauri/target',      # legacy / per-crate target if present
    'src-tauri/gen',         # Tauri generated files (schemas, etc.)
    'tsconfig.tsbuildinfo',  # tsc -b incremental cache
    'node_modules/.vite'     # Vite dep-optimize cache
)

if ($Deps -or $All) {
    $targets += 'node_modules'
}
if ($All) {
    $targets += 'src-tauri/WixTools'   # downloaded WiX installer toolchain
}

function Get-PathSize {
    param([string]$Path)
    try {
        if (Test-Path $Path -PathType Leaf) {
            return (Get-Item -LiteralPath $Path).Length
        }
        $sum = Get-ChildItem -LiteralPath $Path -Recurse -Force -File -ErrorAction SilentlyContinue |
            Measure-Object -Property Length -Sum
        return [int64]($sum.Sum)
    } catch {
        return [int64]0
    }
}

function Format-Size {
    param([int64]$Bytes)
    if ($Bytes -ge 1GB) { return ('{0:N2} GB' -f ($Bytes / 1GB)) }
    if ($Bytes -ge 1MB) { return ('{0:N1} MB' -f ($Bytes / 1MB)) }
    if ($Bytes -ge 1KB) { return ('{0:N0} KB' -f ($Bytes / 1KB)) }
    return "$Bytes B"
}

Write-Host "Cleaning build artifacts in $root" -ForegroundColor Cyan

$freed = [int64]0
$removed = 0
$failures = 0

foreach ($rel in $targets) {
    $full = Join-Path $root $rel
    if (-not (Test-Path -LiteralPath $full)) {
        Write-Host ("  skip    {0}" -f $rel) -ForegroundColor DarkGray
        continue
    }

    $size = if ($Stats) { Get-PathSize $full } else { [int64]0 }

    try {
        Remove-Item -LiteralPath $full -Recurse -Force -ErrorAction Stop
        $removed++
        $freed += $size
        if ($Stats) {
            Write-Host ("  removed {0}  ({1})" -f $rel, (Format-Size $size)) -ForegroundColor Green
        } else {
            Write-Host ("  removed {0}" -f $rel) -ForegroundColor Green
        }
    } catch {
        $failures++
        # Most common cause: a file is locked by a running `tauri dev` / editor.
        Write-Host ("  FAILED  {0} -> {1}" -f $rel, $_.Exception.Message) -ForegroundColor Yellow
    }
}

if ($Stats) {
    Write-Host ("Done. Removed {0} item(s), freed {1}." -f $removed, (Format-Size $freed)) -ForegroundColor Cyan
} else {
    Write-Host ("Done. Removed {0} item(s)." -f $removed) -ForegroundColor Cyan
}

if ($failures -gt 0) {
    throw "Failed to remove $failures target(s)."
}
