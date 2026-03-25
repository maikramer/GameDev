#!/usr/bin/env python3
"""
Texture2D — instalador system-wide (paridade com Text2D).

Instalação automatizada para uso local ou com prefixo (--prefix ~/.local).
"""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Optional

try:
    from rich import box
    from rich.console import Console
    from rich.panel import Panel
    from rich.table import Table

    _RICH = True
except ImportError:
    _RICH = False

_console = Console() if _RICH else None


class Logger:
    """Saída com Rich quando disponível; fallback ANSI simples."""

    @staticmethod
    def info(msg: str) -> None:
        if _RICH and _console:
            _console.print(f"[bold green]INFO[/bold green] {msg}")
        else:
            print(f"\033[0;32m[INFO]\033[0m {msg}")

    @staticmethod
    def warn(msg: str) -> None:
        if _RICH and _console:
            _console.print(f"[bold yellow]WARN[/bold yellow] {msg}")
        else:
            print(f"\033[1;33m[WARN]\033[0m {msg}")

    @staticmethod
    def error(msg: str) -> None:
        if _RICH and _console:
            _console.print(f"[bold red]ERROR[/bold red] {msg}")
        else:
            print(f"\033[0;31m[ERROR]\033[0m {msg}")

    @staticmethod
    def step(msg: str) -> None:
        if _RICH and _console:
            _console.print(f"[bold blue]STEP[/bold blue] {msg}")
        else:
            print(f"\033[0;34m[STEP]\033[0m {msg}")


def _project_root() -> Path:
    """Raiz do repositório (parent de scripts/)."""
    return Path(__file__).resolve().parent.parent


class Texture2DInstaller:
    """Instalador do Texture2D."""

    def __init__(self, args: argparse.Namespace):
        self.args = args
        self.script_dir = _project_root()
        self.venv_dir = self.script_dir / ".venv"

        self.install_prefix = Path(args.prefix)
        self.python_cmd = args.python
        self.use_venv = args.use_venv
        self.skip_deps = args.skip_deps
        self.force = args.force

        self.venv_python = self.venv_dir / "bin" / "python"
        self.venv_exists = self.venv_python.is_file()

        self.logger = Logger()
        self.requirements_file = self.script_dir / "config" / "requirements.txt"

    def run(self) -> bool:
        if _RICH and _console:
            t = Table(show_header=False, box=box.SIMPLE, title="[bold cyan]Texture2D — instalador")
            t.add_row("Prefixo", str(self.install_prefix))
            t.add_row("Python", self.python_cmd)
            t.add_row("Projeto", str(self.script_dir))
            _console.print(Panel(t, border_style="cyan"))
        else:
            self.logger.info(f"Prefixo de instalação: {self.install_prefix}")
            self.logger.info(f"Python: {self.python_cmd}")
            self.logger.info(f"Raiz do projeto: {self.script_dir}")

        if not self.check_python():
            return False

        if not self.skip_deps:
            self.install_system_deps()

        if self.use_venv:
            if not self.venv_exists:
                self.logger.error(
                    f"Não existe venv em {self.venv_dir}. "
                    "Execute ./scripts/setup.sh ou crie .venv e tente de novo; "
                    "ou instale sem --use-venv."
                )
                return False
            self.logger.info(f"Usando venv existente: {self.venv_dir}")
            self.install_in_venv()
        else:
            self.install_system_wide()

        self.create_wrappers()
        self.setup_directories()
        self.show_summary()
        return True

    def check_python(self) -> bool:
        self.logger.step("Verificando Python...")
        try:
            result = subprocess.run(
                [self.python_cmd, "--version"],
                capture_output=True,
                text=True,
                check=True,
            )
            version_str = (result.stdout or result.stderr or "").strip()
            self.logger.info(f"Python detectado: {version_str}")

            ok = subprocess.run(
                [
                    self.python_cmd,
                    "-c",
                    "import sys; print('OK' if sys.version_info >= (3, 10) else 'FAIL')",
                ],
                capture_output=True,
                text=True,
                check=True,
            )
            if "OK" not in ok.stdout:
                self.logger.error("Python 3.10+ necessário")
                return False
            return True
        except (subprocess.CalledProcessError, FileNotFoundError) as e:
            self.logger.error(f"Python não encontrado: {e}")
            return False

    def install_system_deps(self) -> None:
        self.logger.step("Dependências do sistema...")
        if shutil.which("apt-get"):
            self.logger.info("Detectado: Debian/Ubuntu")
            self.logger.warn("Opcional: sudo apt-get install python3-dev python3-venv git")
        elif shutil.which("dnf"):
            self.logger.info("Detectado: Fedora")
            self.logger.warn("Opcional: sudo dnf install python3-devel python3-pip git")
        elif shutil.which("pacman"):
            self.logger.info("Detectado: Arch Linux")
            self.logger.warn("Opcional: sudo pacman -S python python-pip git base-devel")
        else:
            self.logger.warn("Gerenciador de pacotes não reconhecido")

    def install_in_venv(self) -> None:
        self.logger.step("Instalando no venv existente...")
        python = str(self.venv_python)

        if not self.force:
            try:
                subprocess.run(
                    [python, "-c", "import texture2d"],
                    capture_output=True,
                    check=True,
                )
                self.logger.warn("Texture2D já instalado no venv")
                self.logger.info("Use --force para reinstalar")
                return
            except subprocess.CalledProcessError:
                pass

        pip_cmd = [python, "-m", "pip", "install"]
        self.logger.info("Instalando pacote em modo editável...")
        subprocess.run(pip_cmd + ["-e", str(self.script_dir)], check=True)
        self.logger.info("\u2713 Instalado no venv")

    def install_system_wide(self) -> None:
        self.logger.step("Instalando Texture2D (system-wide / prefix)...")
        pip_cmd = [self.python_cmd, "-m", "pip", "install"]

        subprocess.run(
            [self.python_cmd, "-m", "pip", "install", "--upgrade", "pip", "setuptools", "wheel"],
            check=True,
        )

        if self.requirements_file.is_file():
            self.logger.info(f"Instalando dependências: {self.requirements_file}")
            subprocess.run(pip_cmd + ["-r", str(self.requirements_file)], check=True)
        else:
            self.logger.warn(f"Ficheiro em falta: {self.requirements_file}")

        self.logger.info("Instalando pacote texture2d em modo editável...")
        subprocess.run(pip_cmd + ["-e", str(self.script_dir)], check=True)
        self.logger.info("\u2713 Instalação concluída")

    def create_wrappers(self) -> None:
        self.logger.step("Wrappers em bin/...")
        bin_dir = self.install_prefix / "bin"
        bin_dir.mkdir(parents=True, exist_ok=True)

        if self.venv_exists and self.use_venv:
            python_path = str(self.venv_python)
        else:
            python_path = self.python_cmd

        wrapper = bin_dir / "texture2d"
        with open(wrapper, "w", encoding="utf-8") as f:
            f.write("#!/bin/bash\n")
            f.write("# Texture2D — gerado por installer.py\n")
            f.write(f'exec "{python_path}" -m texture2d "$@"\n')
        wrapper.chmod(0o755)
        self.logger.info(f"\u2713 {wrapper}")

        wrapper_gen = bin_dir / "texture2d-generate"
        with open(wrapper_gen, "w", encoding="utf-8") as f:
            f.write("#!/bin/bash\n")
            f.write(f'exec "{bin_dir}/texture2d" generate "$@"\n')
        wrapper_gen.chmod(0o755)

        if self.venv_exists and self.use_venv:
            wrapper_act = bin_dir / "texture2d-activate"
            with open(wrapper_act, "w", encoding="utf-8") as f:
                f.write("#!/bin/bash\n")
                f.write(f'source "{self.venv_dir}/bin/activate"\n')
                f.write('exec "$@"\n')
            wrapper_act.chmod(0o755)
            self.logger.info(f"\u2713 {wrapper_act}")

    def setup_directories(self) -> None:
        out = Path.home() / ".texture2d" / "outputs"
        (out / "textures").mkdir(parents=True, exist_ok=True)
        self.logger.info(f"Diretórios de saída: {out}")

    def show_summary(self) -> None:
        if _RICH and _console:
            lines = [
                "[bold]Comandos[/bold]",
                "  [cyan]texture2d --help[/cyan]",
                '  [cyan]texture2d generate "rough stone wall" -o stone.png[/cyan]',
                "  [cyan]texture2d presets[/cyan]",
                "  [cyan]texture2d info[/cyan]",
                "",
                "[dim]Dev:[/dim] pip install -e \".[dev]\" && pytest tests/ -v",
                "",
                f"Saída: [green]~/.texture2d/outputs/[/green]",
                f"Binários: [green]{self.install_prefix / 'bin'}[/green]",
            ]
            if self.venv_exists and self.use_venv:
                lines.append(f"venv: [green]{self.venv_dir}[/green]")
            w = shutil.which("texture2d")
            lines.append("")
            if w:
                lines.append(f"texture2d no PATH: [bold green]{w}[/bold green]")
            else:
                lines.append(
                    f'[yellow]Adiciona ao PATH:[/yellow] export PATH="{self.install_prefix}/bin:$PATH"'
                )
            _console.print(
                Panel(
                    "\n".join(lines),
                    title="[bold green]Texture2D — instalação concluída",
                    border_style="green",
                )
            )
            return

        print("\n" + "=" * 42)
        print("  Texture2D — instalação concluída")
        print("=" * 42 + "\n")

        if self.venv_exists and self.use_venv:
            print(f"\u2713 venv: {self.venv_dir}\n")

        print("Comandos:")
        print("  texture2d --help")
        print('  texture2d generate "rough stone wall" -o stone.png')
        print("  texture2d presets")
        print("  texture2d info")
        print()
        print("  Testes (dev): pip install -e \".[dev]\" && pytest tests/ -v")
        print()
        print("  Saída padrão: ~/.texture2d/outputs/")
        print(f"  Binários: {self.install_prefix}/bin/")
        print()

        w = shutil.which("texture2d")
        if w:
            print(f"\u2713 texture2d no PATH: {w}")
        else:
            print(f'\u26a0 Adicione ao PATH: export PATH="{self.install_prefix}/bin:$PATH"')
        print()


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Texture2D — instalador",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Exemplos:
  python3 scripts/installer.py --use-venv
  python3 scripts/installer.py --prefix ~/.local
  sudo python3 scripts/installer.py --prefix /usr/local
  python3 scripts/installer.py --use-venv --skip-deps --force

Variáveis:
  INSTALL_PREFIX   diretório de instalação (binários)
  PYTHON_CMD       interpretador Python (default: python3)

Nota: Texture2D não necessita de PyTorch — usa HF Inference API (cloud).
        """,
    )
    parser.add_argument(
        "--prefix",
        default=os.environ.get("INSTALL_PREFIX", str(Path.home() / ".local")),
        help="Prefixo de instalação (default: ~/.local)",
    )
    parser.add_argument("--use-venv", action="store_true", help="Usar .venv na raiz do projeto")
    parser.add_argument("--skip-deps", action="store_true", help="Avisos mínimos de sistema")
    parser.add_argument("--force", action="store_true", help="Reinstalar mesmo se já existir")
    parser.add_argument(
        "--python",
        default=os.environ.get("PYTHON_CMD", "python3"),
        help="Comando Python",
    )

    args = parser.parse_args()

    venv_dir = _project_root() / ".venv"
    if venv_dir.is_dir() and not args.use_venv:
        msg = f"Venv em {venv_dir} — use --use-venv para instalação rápida"
        if _RICH and _console:
            _console.print(f"[yellow]{msg}[/yellow]\n")
        else:
            print(f"[INFO] {msg}\n")

    installer = Texture2DInstaller(args)
    sys.exit(0 if installer.run() else 1)


if __name__ == "__main__":
    main()
