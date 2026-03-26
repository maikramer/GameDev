@echo off
REM GameDev Monorepo — Instalador Unificado (Windows CMD)
REM Uso: install.bat <tool> [opcoes]

setlocal EnableDelayedExpansion

echo GameDev Monorepo — Instalador Unificado
echo ========================================

python --version >nul 2>&1
if errorlevel 1 (
    python3 --version >nul 2>&1
    if errorlevel 1 (
        echo Python 3 nao encontrado. Instale de https://python.org
        exit /b 1
    )
    set PY=python3
) else (
    set PY=python
)

set "SCRIPT_DIR=%~dp0"
set "PYTHONPATH=%SCRIPT_DIR%Shared\src;%PYTHONPATH%"

%PY% -m gamedev_shared.installer.unified --python %PY% %*
