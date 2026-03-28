# Text3D — setup no Windows (PowerShell)
# Uso: .\scripts\setup.ps1   (na raiz Text3D)
# Requer: Python 3.10+, Git, CUDA Toolkit (nvcc) opcional para custom_rasterizer

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $Root

$Venv = Join-Path $Root ".venv"
$Py = if ($env:PYTHON_CMD) { $env:PYTHON_CMD } else { "python" }

Write-Host "Text3D — setup (Windows)" -ForegroundColor Green

if (-not (Get-Command $Py -ErrorAction SilentlyContinue)) {
    Write-Error "Python não encontrado. Defina PYTHON_CMD ou instale Python 3.10+."
}

& $Py -m venv $Venv
$VenvPy = Join-Path $Venv "Scripts\python.exe"
& $VenvPy -m pip install --upgrade pip "setuptools>=68,<82" wheel
& $VenvPy -m pip install -e $Root

Write-Host "Opcional: pip install spandrel (upscale de textura)" -ForegroundColor Yellow
Write-Host "Opcional: compilar custom_rasterizer (Paint) — CUDA + Git; ver docs/PAINT_SETUP.md" -ForegroundColor Yellow
Write-Host "Ativar:  .\.venv\Scripts\Activate.ps1" -ForegroundColor Cyan
Write-Host "Testar:  text3d doctor" -ForegroundColor Cyan
