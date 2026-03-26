"""PythonProjectInstaller — instalador para projectos Python do monorepo."""

from __future__ import annotations

import subprocess
from pathlib import Path
from typing import Optional

from .base import BaseInstaller

# Política do monorepo: o pacote do projecto é sempre instalado com
# ``pip install -e`` (modo editável). Alterações ao código-fonte refletem-se
# no venv ou no prefix — não há modo wheel-only neste instalador.


class PythonProjectInstaller(BaseInstaller):
    """Instalador para projectos Python (sempre ``pip install -e``).

    Política:
    - Cria ``projecto/.venv`` se não existir e instala **sempre** nesse venv.
    - Os wrappers em ``bin_dir`` apontam para ``.venv/bin/python`` (ou ``Scripts`` no Windows).
    - ``install_system_wide`` mantém-se só para subclasses/testes; o fluxo normal não o usa.
    """

    def __init__(
        self,
        *,
        project_name: str,
        cli_name: str,
        project_root: Path,
        install_prefix: Path | None = None,
        python_cmd: str = "python3",
        use_venv: bool = False,
        skip_deps: bool = False,
        skip_models: bool = False,
        force: bool = False,
        skip_pytorch: bool = False,
    ) -> None:
        super().__init__(
            project_name=project_name,
            cli_name=cli_name,
            project_root=project_root,
            install_prefix=install_prefix,
            python_cmd=python_cmd,
        )
        self.use_venv = use_venv
        self.skip_deps = skip_deps
        self.skip_models = skip_models
        self.force = force
        self.skip_pytorch = skip_pytorch

        self.venv_dir = self.project_root / ".venv"
        if self.is_windows:
            self.venv_python = self.venv_dir / "Scripts" / "python.exe"
        else:
            self.venv_python = self.venv_dir / "bin" / "python"
        self.venv_exists = self.venv_python.is_file()
        self.requirements_file = self.project_root / "config" / "requirements.txt"

    # ------------------------------------------------------------------
    # Fluxo principal
    # ------------------------------------------------------------------

    def run(self) -> bool:
        self.logger.table(
            [
                ("Prefixo", str(self.install_prefix)),
                ("Python", self.python_cmd),
                ("Projeto", str(self.project_root)),
            ],
            title=f"{self.project_name} — instalador",
        )

        if not self.check_python():
            return False

        if not self.skip_deps:
            self.install_system_deps()

        if not self.ensure_project_venv():
            return False

        self.install_in_venv()

        return True

    # ------------------------------------------------------------------
    # venv do projecto
    # ------------------------------------------------------------------

    def ensure_project_venv(self) -> bool:
        """Garante ``projecto/.venv`` com um interpretador válido (cria se necessário)."""
        if self.venv_python.is_file():
            self.venv_exists = True
            self.logger.info(f"Venv do projecto: {self.venv_dir}")
            return True

        self.logger.step(f"Criando ambiente virtual em {self.venv_dir}...")
        try:
            subprocess.run(
                [self.python_cmd, "-m", "venv", str(self.venv_dir)],
                check=True,
            )
        except subprocess.CalledProcessError as e:
            self.logger.error(f"Falha ao criar venv: {e}")
            if not self.is_windows:
                self.logger.info("Em Debian/Ubuntu: sudo apt install python3-venv")
            return False

        if not self.venv_python.is_file():
            self.logger.error(f"Python não encontrado após criar venv: {self.venv_python}")
            return False

        self.venv_exists = True
        self.logger.success(f"Venv criado: {self.venv_dir}")
        return True

    # ------------------------------------------------------------------
    # Instalação em venv
    # ------------------------------------------------------------------

    def install_in_venv(self) -> None:
        """Instala dependências e o pacote em modo editável no ``.venv`` do projecto."""
        self.logger.step("Instalando no venv do projecto...")
        python = str(self.venv_python)
        pip_cmd = [python, "-m", "pip", "install"]

        _root = str(self.project_root)
        subprocess.run(
            [python, "-m", "pip", "install", "--upgrade", "pip", "setuptools", "wheel"],
            check=True,
            cwd=_root,
        )

        if not self.force:
            try:
                subprocess.run(
                    [python, "-c", f"import {self.cli_name}"],
                    capture_output=True,
                    check=True,
                )
                self.logger.warn(f"{self.project_name} já instalado no venv")
                self.logger.info("Use --force para reinstalar")
                return
            except subprocess.CalledProcessError:
                pass

        if not self.skip_pytorch:
            self.install_pytorch(pip_cmd, cwd=self.project_root)

        if self.requirements_file.is_file():
            self.logger.info(f"Instalando dependências: {self.requirements_file}")
            subprocess.run(
                pip_cmd + ["-r", str(self.requirements_file)],
                check=True,
                cwd=_root,
            )
        else:
            self.logger.warn(f"Ficheiro em falta: {self.requirements_file}")

        self.logger.info("Instalando pacote em modo editável...")
        subprocess.run(
            pip_cmd + ["-e", str(self.project_root)],
            check=True,
            cwd=_root,
        )
        self.logger.success("Instalado no venv")

    # ------------------------------------------------------------------
    # Instalação system-wide
    # ------------------------------------------------------------------

    def install_system_wide(self) -> None:
        self.logger.step(f"Instalando {self.project_name} (system-wide / prefix)...")
        pip_cmd = [self.python_cmd, "-m", "pip", "install"]
        _root = str(self.project_root)

        subprocess.run(
            [self.python_cmd, "-m", "pip", "install", "--upgrade", "pip", "setuptools", "wheel"],
            check=True,
            cwd=_root,
        )

        if not self.skip_pytorch:
            self.install_pytorch(pip_cmd, cwd=self.project_root)

        if self.requirements_file.is_file():
            self.logger.info(f"Instalando dependências: {self.requirements_file}")
            subprocess.run(
                pip_cmd + ["-r", str(self.requirements_file)],
                check=True,
                cwd=_root,
            )
        else:
            self.logger.warn(f"Ficheiro em falta: {self.requirements_file}")

        self.logger.info(f"Instalando pacote {self.cli_name} em modo editável...")
        subprocess.run(
            pip_cmd + ["-e", str(self.project_root)],
            check=True,
            cwd=_root,
        )
        self.logger.success("Instalação concluída")

    # ------------------------------------------------------------------
    # Wrappers de conveniência
    # ------------------------------------------------------------------

    def create_cli_wrappers(self, extra_aliases: list[str] | None = None) -> None:
        """Cria wrapper principal e aliases opcionais.

        Os wrappers em ``bin_dir`` usam sempre o Python de ``projecto/.venv`` quando
        esse venv existe — nunca um interpretador arbitrário de outro ambiente.
        """
        python_path = str(self.venv_python) if self.venv_exists else self.python_cmd
        self.create_wrapper(
            self.cli_name,
            python_path=python_path,
            module_name=self.cli_name,
        )
        mod = self.cli_name
        for alias in extra_aliases or []:
            if self.is_windows:
                w = self.bin_dir / f"{alias}.cmd"
                with open(w, "w", encoding="utf-8", newline="\r\n") as f:
                    f.write("@echo off\r\n")
                    f.write(f'"{python_path}" -m {mod} generate %*\r\n')
                self.logger.success(str(w))
            else:
                wrapper = self.bin_dir / alias
                with open(wrapper, "w", encoding="utf-8") as f:
                    f.write("#!/bin/bash\n")
                    f.write(f'exec "{self.bin_dir}/{self.cli_name}" generate "$@"\n')
                wrapper.chmod(0o755)
                self.logger.success(str(wrapper))

    def create_activate_wrapper(self) -> Optional[Path]:
        """Cria wrapper que activa o venv (para desenvolvimento)."""
        if self.is_windows:
            return None
        if not self.venv_exists:
            return None
        wrapper = self.bin_dir / f"{self.cli_name}-activate"
        with open(wrapper, "w", encoding="utf-8") as f:
            f.write("#!/bin/bash\n")
            f.write(f'source "{self.venv_dir}/bin/activate"\n')
            f.write('exec "$@"\n')
        wrapper.chmod(0o755)
        self.logger.success(str(wrapper))
        return wrapper
