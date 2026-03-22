# Materialize CLI Installer - Windows PowerShell
#
# Installs Materialize CLI (generates PBR maps from textures).
# Requirements: PowerShell 5.1+, Python 3 (installer), Rust/cargo (to build).
#
# Usage:
#   .\install.ps1              # Install
#   .\install.ps1 uninstall   # Uninstall
#   .\install.ps1 reinstall   # Reinstall
#

param(
    [Parameter(Position=0)]
    [ValidateSet('install', 'uninstall', 'reinstall')]
    [string]$Action = 'install'
)

$ErrorActionPreference = "Stop"

$Cyan = "`e[36m"
$Green = "`e[32m"
$Red = "`e[31m"
$Reset = "`e[0m"

Write-Host "${Cyan}🚀 Materialize CLI Installer${Reset}"
Write-Host "================================"

$pythonCmd = Get-Command python -ErrorAction SilentlyContinue
if (-not $pythonCmd) {
    $pythonCmd = Get-Command python3 -ErrorAction SilentlyContinue
}
if (-not $pythonCmd) {
    Write-Host "${Red}✗ Python not found${Reset}"
    Write-Host "Install Python 3 from https://python.org"
    exit 1
}

Write-Host "${Green}✓ Python: $($pythonCmd.Source)${Reset}"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $scriptDir

& $pythonCmd.Source installer/installer.py $Action
