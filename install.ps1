# =============================================================================
# GameDev Monorepo — Instalador Unificado (Windows PowerShell)
# =============================================================================
#
# Instala qualquer ferramenta do monorepo GameDev.
#
# Uso:
#   .\install.ps1 materialize           # Instalar Materialize (Rust)
#   .\install.ps1 text2d --use-venv     # Instalar Text2D no venv
#   .\install.ps1 all                   # Instalar tudo
#   .\install.ps1 --list                # Listar ferramentas
#
# =============================================================================

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$SharedSrc = Join-Path $ScriptDir "Shared" "src"

$Cyan = "`e[36m"
$Green = "`e[32m"
$Red = "`e[31m"
$Reset = "`e[0m"

$pythonCmd = Get-Command python -ErrorAction SilentlyContinue
if (-not $pythonCmd) {
    $pythonCmd = Get-Command python3 -ErrorAction SilentlyContinue
}
if (-not $pythonCmd) {
    Write-Host "${Red}Python 3 nao encontrado. Instale de https://python.org${Reset}"
    exit 1
}

Write-Host "${Cyan}GameDev Monorepo — Instalador Unificado${Reset}"
Write-Host "========================================"

$env:PYTHONPATH = "$SharedSrc;$($env:PYTHONPATH)"

& $pythonCmd.Source -m gamedev_shared.installer.unified @args
