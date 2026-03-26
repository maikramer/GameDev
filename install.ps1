# =============================================================================
# GameDev Monorepo — Instalador Unificado (Windows PowerShell)
# =============================================================================
#
# Instala qualquer ferramenta do monorepo GameDev.
#
# Uso:
#   .\install.ps1 materialize           # Instalar Materialize (Rust)
#   .\install.ps1 text2d              # com .venv no projecto, instala no venv do projecto
#   .\install.ps1 all                   # Instalar tudo
#   .\install.ps1 --list                # Listar ferramentas
#
# =============================================================================

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$SharedRoot = Join-Path $ScriptDir "Shared"
$SharedSrc = Join-Path $SharedRoot "src"

$Cyan = "`e[36m"
$Red = "`e[31m"
$Reset = "`e[0m"

function Prepare-InstallerEnvironment {
    Write-Host "${Cyan}Preparando ambiente do instalador...${Reset}"

    $pkgPath = Join-Path $SharedSrc "gamedev_shared"
    if (-not (Test-Path -LiteralPath $pkgPath)) {
        Write-Host "${Red}Monorepo incompleto: nao existe $pkgPath${Reset}"
        Write-Host "  Clona o repositorio completo (pasta Shared/ e obrigatoria)."
        exit 1
    }

    $pythonCmd = Get-Command python -ErrorAction SilentlyContinue
    if (-not $pythonCmd) {
        $pythonCmd = Get-Command python3 -ErrorAction SilentlyContinue
    }
    if (-not $pythonCmd) {
        Write-Host "${Red}Python 3 nao encontrado. Instale de https://python.org${Reset}"
        exit 1
    }

    $py = $pythonCmd.Source
    & $py -c "import sys; assert sys.version_info >= (3, 10)" 2>$null
    if ($LASTEXITCODE -ne 0) {
        Write-Host "${Red}Python 3.10 ou superior e necessario.${Reset}"
        & $py -V 2>$null
        exit 1
    }

    $env:PYTHONPATH = "$SharedSrc;$($env:PYTHONPATH)"

    & $py -c "import rich" 2>$null
    if ($LASTEXITCODE -ne 0) {
        Write-Host "${Cyan}  -> pip: dependencias do instalador (Rich)...${Reset}"
        $req = Join-Path $SharedRoot "config\requirements.txt"
        & $py -m pip install -q -r $req
        if ($LASTEXITCODE -ne 0) {
            Write-Host "${Red}Falha ao instalar dependencias do instalador.${Reset}"
            Write-Host "  Tenta manualmente: $py -m pip install -r Shared\config\requirements.txt"
            exit 1
        }
    }

    return $py
}

$PythonExe = Prepare-InstallerEnvironment

Write-Host "${Cyan}GameDev Monorepo — Instalador Unificado${Reset}"
Write-Host "========================================"

& $PythonExe -m gamedev_shared.installer.unified @args
exit $LASTEXITCODE
