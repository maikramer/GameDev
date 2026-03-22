@echo off
REM Materialize CLI Installer - Windows CMD
REM Installs Materialize CLI (generates PBR maps from textures).

setlocal EnableDelayedExpansion

echo Materialize CLI Installer
echo ================================

python --version >nul 2>&1
if errorlevel 1 (
    python3 --version >nul 2>&1
    if errorlevel 1 (
        echo Python not found. Install Python 3 from https://python.org
        exit /b 1
    )
    set PY=python3
) else (
    set PY=python
)

echo OK Python found

cd /d "%~dp0"
%PY% installer\installer.py %*
