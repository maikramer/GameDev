"""PythonProjectInstaller — instalador para projectos Python do monorepo."""

from __future__ import annotations

import subprocess
from pathlib import Path
from typing import Optional

from .base import BaseInstaller


class PythonProjectInstaller(BaseInstaller):
    """Instalador para projectos Python (pip install -e, venv, requirements).

    Estende ``BaseInstaller`` com:
    - Gestão de venv existente
    - Instalação via pip (editável ou system-wide)
    - Ficheiro de requirements
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

        if self.use_venv:
            if not self.venv_exists:
                self.logger.error(
                    f"Não existe venv em {self.venv_dir}. "
                    "Execute scripts/setup.sh ou crie .venv; ou instale sem --use-venv."
                )
                return False
            self.logger.info(f"Usando venv existente: {self.venv_dir}")
            self.install_in_venv()
        else:
            self.install_system_wide()

        return True

    # ------------------------------------------------------------------
    # Instalação em venv
    # ------------------------------------------------------------------

    def install_in_venv(self) -> None:
        self.logger.step("Instalando no venv existente...")
        python = str(self.venv_python)

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

        pip_cmd = [python, "-m", "pip", "install"]
        self.logger.info("Instalando pacote em modo editável...")
        subprocess.run(pip_cmd + ["-e", str(self.project_root)], check=True)
        self.logger.success("Instalado no venv")

    # ------------------------------------------------------------------
    # Instalação system-wide
    # ------------------------------------------------------------------

    def install_system_wide(self) -> None:
        self.logger.step(f"Instalando {self.project_name} (system-wide / prefix)...")
        pip_cmd = [self.python_cmd, "-m", "pip", "install"]

        subprocess.run(
            [self.python_cmd, "-m", "pip", "install", "--upgrade", "pip", "setuptools", "wheel"],
            check=True,
        )

        if not self.skip_pytorch:
            self.install_pytorch(pip_cmd)

        if self.requirements_file.is_file():
            self.logger.info(f"Instalando dependências: {self.requirements_file}")
            subprocess.run(pip_cmd + ["-r", str(self.requirements_file)], check=True)
        else:
            self.logger.warn(f"Ficheiro em falta: {self.requirements_file}")

        self.logger.info(f"Instalando pacote {self.cli_name} em modo editável...")
        subprocess.run(pip_cmd + ["-e", str(self.project_root)], check=True)
        self.logger.success("Instalação concluída")

    # ------------------------------------------------------------------
    # Wrappers de conveniência
    # ------------------------------------------------------------------

    def create_cli_wrappers(self, extra_aliases: list[str] | None = None) -> None:
        """Cria wrapper principal e aliases opcionais."""
        python_path = str(self.venv_python) if (self.venv_exists and self.use_venv) else self.python_cmd
        self.create_wrapper(
            self.cli_name,
            python_path=python_path,
            module_name=self.cli_name,
        )
        for alias in extra_aliases or []:
            wrapper = self.bin_dir / alias
            with open(wrapper, "w", encoding="utf-8") as f:
                f.write("#!/bin/bash\n")
                f.write(f'exec "{self.bin_dir}/{self.cli_name}" generate "$@"\n')
            wrapper.chmod(0o755)
            self.logger.success(str(wrapper))

    def create_activate_wrapper(self) -> Optional[Path]:
        """Cria wrapper que activa o venv (para desenvolvimento)."""
        if not (self.venv_exists and self.use_venv):
            return None
        wrapper = self.bin_dir / f"{self.cli_name}-activate"
        with open(wrapper, "w", encoding="utf-8") as f:
            f.write("#!/bin/bash\n")
            f.write(f'source "{self.venv_dir}/bin/activate"\n')
            f.write('exec "$@"\n')
        wrapper.chmod(0o755)
        self.logger.success(str(wrapper))
        return wrapper
