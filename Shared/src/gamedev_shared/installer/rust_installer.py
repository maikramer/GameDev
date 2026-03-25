"""RustProjectInstaller — instalador para projectos Rust do monorepo."""

from __future__ import annotations

import os
import platform
import shutil
import subprocess
from pathlib import Path
from typing import Optional

from .base import BaseInstaller


class RustProjectInstaller(BaseInstaller):
    """Instalador para projectos Rust (cargo build + cópia de binário).

    Estende ``BaseInstaller`` com:
    - Verificação de cargo/rustc
    - Build release
    - Cópia do binário para bin_dir
    - Uninstall
    """

    def __init__(
        self,
        *,
        project_name: str,
        cli_name: str,
        project_root: Path,
        cargo_bin_name: str,
        install_prefix: Path | None = None,
    ) -> None:
        super().__init__(
            project_name=project_name,
            cli_name=cli_name,
            project_root=project_root,
            install_prefix=install_prefix,
        )
        self.cargo_bin_name = cargo_bin_name
        ext = ".exe" if self.is_windows else ""
        self.release_binary = self.project_root / "target" / "release" / f"{cargo_bin_name}{ext}"
        self.debug_binary = self.project_root / "target" / "debug" / f"{cargo_bin_name}{ext}"
        self.installed_binary = self.bin_dir / (f"{cli_name}{ext}")

    # ------------------------------------------------------------------
    # Verificações
    # ------------------------------------------------------------------

    def check_cargo(self) -> bool:
        try:
            result = subprocess.run(
                ["cargo", "--version"],
                capture_output=True,
                text=True,
                check=True,
                timeout=5,
            )
            self.logger.success(f"Rust: {result.stdout.strip()}")
            return True
        except (subprocess.CalledProcessError, FileNotFoundError):
            self.logger.warn("cargo não encontrado no PATH")
            return False

    def get_existing_binary(self) -> Optional[Path]:
        if self.release_binary.exists():
            return self.release_binary
        if self.debug_binary.exists():
            return self.debug_binary
        return None

    # ------------------------------------------------------------------
    # Build
    # ------------------------------------------------------------------

    def build_release(self, timeout: int = 600) -> bool:
        self.logger.header(f"Building {self.project_name} (release)")
        try:
            subprocess.run(
                ["cargo", "build", "--release"],
                cwd=self.project_root,
                check=True,
                timeout=timeout,
            )
            self.logger.success(f"Build: target/release/{self.cargo_bin_name}")
            return True
        except subprocess.CalledProcessError as e:
            self.logger.error(f"Build falhou: {e}")
            return False
        except FileNotFoundError:
            self.logger.error("cargo não encontrado. Instale Rust: https://rustup.rs")
            return False
        except subprocess.TimeoutExpired:
            self.logger.error("Build timeout")
            return False

    # ------------------------------------------------------------------
    # Instalar / desinstalar binário
    # ------------------------------------------------------------------

    def install_binary(self) -> bool:
        """Copia binário para bin_dir (build se necessário)."""
        self.logger.header("Instalando binário")

        src = self.get_existing_binary()
        if not src:
            if not self.check_cargo():
                return False
            if not self.build_release():
                return False
            src = self.release_binary

        if not src.exists():
            self.logger.error(f"Binário não encontrado: {src}")
            return False

        try:
            self.bin_dir.mkdir(parents=True, exist_ok=True)
            dest = self.installed_binary
            if dest.exists() or dest.is_symlink():
                dest.unlink()
            shutil.copy2(src, dest)
            if not self.is_windows:
                dest.chmod(0o755)
            self.logger.success(f"{self.cli_name} instalado em {dest}")
            return True
        except Exception as e:
            self.logger.error(f"Erro ao instalar: {e}")
            return False

    def uninstall(self) -> bool:
        self.logger.header(f"Desinstalando {self.project_name}")
        try:
            if self.installed_binary.exists():
                self.installed_binary.unlink()
                self.logger.success(f"Removido: {self.installed_binary}")
            self.logger.success(f"{self.project_name} desinstalado.")
            return True
        except Exception as e:
            self.logger.error(f"Erro ao desinstalar: {e}")
            return False

    def test_installation(self) -> bool:
        self.logger.header("Testando instalação")
        try:
            result = subprocess.run(
                [str(self.installed_binary), "--version"],
                capture_output=True,
                text=True,
                timeout=10,
            )
            if result.returncode == 0:
                self.logger.success(f"Versão: {result.stdout.strip()}")
                return True
            self.logger.warn("Retornou código não-zero")
            return True
        except Exception as e:
            self.logger.warn(f"Não foi possível testar: {e}")
            return True

    # ------------------------------------------------------------------
    # Fluxo completo
    # ------------------------------------------------------------------

    def run(self) -> bool:
        """Instala: check deps → build → copy → PATH → test."""
        self.logger.header(f"{self.project_name} Installer")
        self.logger.table(
            [
                ("Plataforma", platform.system()),
                ("Repositório", str(self.project_root)),
                ("Binários", str(self.bin_dir)),
            ],
            title=self.project_name,
        )

        if not self.check_python():
            return False
        if not self.install_binary():
            return False

        self.check_path()
        self.test_installation()

        self.show_summary(
            commands=[
                f"{self.cli_name} texture.png -o ./out/",
                f"{self.cli_name} --help",
            ],
        )
        return True

    def run_uninstall(self) -> bool:
        return self.uninstall()

    def run_reinstall(self) -> bool:
        self.uninstall()
        return self.run()
