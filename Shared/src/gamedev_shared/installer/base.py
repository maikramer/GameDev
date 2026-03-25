"""BaseInstaller — lógica partilhada entre instaladores Python e Rust."""

from __future__ import annotations

import argparse
import os
import platform
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Optional

from ..logging import Logger


class BaseInstaller:
    """Classe base para instaladores do monorepo GameDev.

    Fornece:
    - Detecção de plataforma
    - Verificação de Python
    - Detecção de sistema de pacotes Linux
    - Instalação de PyTorch (CUDA/CPU)
    - Criação de wrappers bash
    - Sumário de instalação
    """

    def __init__(
        self,
        *,
        project_name: str,
        cli_name: str,
        project_root: Path,
        install_prefix: Path | None = None,
        python_cmd: str = "python3",
    ) -> None:
        self.project_name = project_name
        self.cli_name = cli_name
        self.project_root = project_root.resolve()
        self.install_prefix = (
            install_prefix
            or Path(os.environ.get("INSTALL_PREFIX", str(Path.home() / ".local")))
        )
        self.python_cmd = os.environ.get("PYTHON_CMD", python_cmd)

        self.plat = platform.system().lower()
        self.is_windows = self.plat == "windows"
        self.is_macos = self.plat == "darwin"
        self.is_linux = self.plat == "linux"

        self.bin_dir = self._default_bin_dir()
        self.logger = Logger()

    def _default_bin_dir(self) -> Path:
        if self.is_windows:
            return Path(os.environ.get("USERPROFILE") or "C:\\") / "bin"
        return self.install_prefix / "bin"

    # ------------------------------------------------------------------
    # Verificação de Python
    # ------------------------------------------------------------------

    def check_python(self, min_version: tuple[int, int] = (3, 10)) -> bool:
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

            major, minor = min_version
            ok = subprocess.run(
                [
                    self.python_cmd,
                    "-c",
                    f"import sys; print('OK' if sys.version_info >= ({major}, {minor}) else 'FAIL')",
                ],
                capture_output=True,
                text=True,
                check=True,
            )
            if "OK" not in ok.stdout:
                self.logger.error(f"Python {major}.{minor}+ necessário")
                return False
            return True
        except (subprocess.CalledProcessError, FileNotFoundError) as e:
            self.logger.error(f"Python não encontrado: {e}")
            return False

    # ------------------------------------------------------------------
    # Dependências do sistema
    # ------------------------------------------------------------------

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

    # ------------------------------------------------------------------
    # PyTorch
    # ------------------------------------------------------------------

    def _python_minor(self) -> int:
        try:
            out = subprocess.run(
                [self.python_cmd, "-c", "import sys; print(sys.version_info[1])"],
                capture_output=True,
                text=True,
                check=True,
            )
            return int((out.stdout or "").strip())
        except (ValueError, subprocess.CalledProcessError):
            return 10

    def install_pytorch(self, pip_cmd: list[str] | None = None) -> None:
        """Instala PyTorch com CUDA (se disponível) ou CPU."""
        if pip_cmd is None:
            pip_cmd = [self.python_cmd, "-m", "pip", "install"]

        has_cuda = shutil.which("nvidia-smi") is not None
        py_minor = self._python_minor()

        if has_cuda:
            try:
                result = subprocess.run(
                    ["nvidia-smi"], capture_output=True, text=True
                )
                if "CUDA Version" in result.stdout:
                    for line in result.stdout.split("\n"):
                        if "CUDA Version" in line:
                            cuda_version = line.split("CUDA Version:")[1].split()[0]
                            self.logger.info(f"CUDA detectado: {cuda_version}")
                            if py_minor >= 13:
                                self.logger.info("Python 3.13+ — torch+torchvision (PyPI)...")
                                subprocess.run(pip_cmd + ["torch", "torchvision"], check=True)
                                return
                            idx = (
                                "https://download.pytorch.org/whl/cu121"
                                if cuda_version.startswith("12")
                                else "https://download.pytorch.org/whl/cu118"
                            )
                            self.logger.info(f"PyTorch ({idx.split('/')[-1]})...")
                            subprocess.run(
                                pip_cmd + ["torch", "torchvision", "--index-url", idx],
                                check=True,
                            )
                            return
            except Exception:
                pass

        self.logger.warn("PyTorch CPU...")
        subprocess.run(
            pip_cmd
            + ["torch", "torchvision", "--index-url", "https://download.pytorch.org/whl/cpu"],
            check=True,
        )

    # ------------------------------------------------------------------
    # Wrappers
    # ------------------------------------------------------------------

    def create_wrapper(
        self,
        bin_name: str,
        *,
        python_path: str | None = None,
        module_name: str | None = None,
        target_binary: Path | None = None,
    ) -> Path:
        """Cria wrapper bash em ``bin_dir``.

        Modo Python (module_name)::
            exec "<python_path>" -m <module_name> "$@"

        Modo binário (target_binary)::
            exec "<target_binary>" "$@"
        """
        self.bin_dir.mkdir(parents=True, exist_ok=True)
        wrapper = self.bin_dir / bin_name
        with open(wrapper, "w", encoding="utf-8") as f:
            f.write("#!/bin/bash\n")
            f.write(f"# {self.project_name} — gerado por installer\n")
            if target_binary:
                f.write(f'exec "{target_binary}" "$@"\n')
            else:
                py = python_path or self.python_cmd
                mod = module_name or self.cli_name
                f.write(f'exec "{py}" -m {mod} "$@"\n')
        wrapper.chmod(0o755)
        self.logger.success(str(wrapper))
        return wrapper

    # ------------------------------------------------------------------
    # PATH
    # ------------------------------------------------------------------

    def check_path(self) -> bool:
        """Verifica e reporta se bin_dir está no PATH."""
        bin_str = str(self.bin_dir)
        path_env = os.environ.get("PATH", "")
        sep = ";" if self.is_windows else ":"

        if bin_str in path_env.split(sep):
            self.logger.success(f"{bin_str} está no PATH")
            return True

        self.logger.warn(f"{bin_str} pode não estar no PATH")
        if not self.is_windows:
            self.logger.info(f'Adicione: export PATH="{bin_str}:$PATH"')
        else:
            self.logger.info(f"Adicione ao PATH do sistema: {bin_str}")
        return False

    # ------------------------------------------------------------------
    # Sumário
    # ------------------------------------------------------------------

    def show_summary(self, commands: list[str], *, extras: list[str] | None = None) -> None:
        """Mostra resumo de instalação com Rich/ANSI."""
        lines = ["[bold]Comandos[/bold]" if self.logger.rich_available else "Comandos:"]
        for cmd in commands:
            if self.logger.rich_available:
                lines.append(f"  [cyan]{cmd}[/cyan]")
            else:
                lines.append(f"  {cmd}")

        if extras:
            lines.append("")
            lines.extend(extras)

        lines.append("")
        found = shutil.which(self.cli_name)
        if found:
            if self.logger.rich_available:
                lines.append(f"{self.cli_name} no PATH: [bold green]{found}[/bold green]")
            else:
                lines.append(f"✓ {self.cli_name} no PATH: {found}")
        else:
            if self.logger.rich_available:
                lines.append(
                    f'[yellow]Adiciona ao PATH:[/yellow] export PATH="{self.bin_dir}:$PATH"'
                )
            else:
                lines.append(f'⚠ Adicione ao PATH: export PATH="{self.bin_dir}:$PATH"')

        self.logger.panel(
            "\n".join(lines),
            title=f"{self.project_name} — instalação concluída",
            border="green",
        )
