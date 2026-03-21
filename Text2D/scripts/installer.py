#!/usr/bin/env python3
"""
Text2D — instalador system-wide (paridade com Text3D).

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


class Colors:
    GREEN = "\033[0;32m"
    YELLOW = "\033[1;33m"
    RED = "\033[0;31m"
    BLUE = "\033[0;34m"
    NC = "\033[0m"


class Logger:
    @staticmethod
    def info(msg: str) -> None:
        print(f"{Colors.GREEN}[INFO]{Colors.NC} {msg}")

    @staticmethod
    def warn(msg: str) -> None:
        print(f"{Colors.YELLOW}[WARN]{Colors.NC} {msg}")

    @staticmethod
    def error(msg: str) -> None:
        print(f"{Colors.RED}[ERROR]{Colors.NC} {msg}")

    @staticmethod
    def step(msg: str) -> None:
        print(f"{Colors.BLUE}[STEP]{Colors.NC} {msg}")


def _project_root() -> Path:
    """Raiz do repositório (parent de scripts/)."""
    return Path(__file__).resolve().parent.parent


class Text2DInstaller:
    """Instalador do Text2D."""

    def __init__(self, args: argparse.Namespace):
        self.args = args
        self.script_dir = _project_root()
        self.venv_dir = self.script_dir / ".venv"

        self.install_prefix = Path(args.prefix)
        self.python_cmd = args.python
        self.use_venv = args.use_venv
        self.skip_deps = args.skip_deps
        self.skip_models = args.skip_models
        self.force = args.force

        self.venv_python = self.venv_dir / "bin" / "python"
        self.venv_exists = self.venv_python.is_file()

        self.logger = Logger()
        self.requirements_file = self.script_dir / "config" / "requirements.txt"

    def run(self) -> bool:
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

        self.setup_models()
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
                    [python, "-c", "import text2d"],
                    capture_output=True,
                    check=True,
                )
                self.logger.warn("Text2D já instalado no venv")
                self.logger.info("Use --force para reinstalar")
                return
            except subprocess.CalledProcessError:
                pass

        pip_cmd = [python, "-m", "pip", "install"]
        self.logger.info("Instalando pacote (editable implícito via setuptools)...")
        subprocess.run(pip_cmd + [str(self.script_dir)], check=True)
        self.logger.info("✓ Instalado no venv")

    def install_system_wide(self) -> None:
        self.logger.step("Instalando Text2D (system-wide / prefix)...")
        pip_cmd = [self.python_cmd, "-m", "pip", "install"]

        subprocess.run(
            [self.python_cmd, "-m", "pip", "install", "--upgrade", "pip", "setuptools", "wheel"],
            check=True,
        )

        self.install_pytorch()

        if self.requirements_file.is_file():
            self.logger.info(f"Instalando dependências: {self.requirements_file}")
            subprocess.run(pip_cmd + ["-r", str(self.requirements_file)], check=True)
        else:
            self.logger.warn(f"Ficheiro em falta: {self.requirements_file}")

        self.logger.info("Instalando pacote text2d...")
        subprocess.run(pip_cmd + [str(self.script_dir)], check=True)
        self.logger.info("✓ Instalação concluída")

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

    def install_pytorch(self) -> None:
        pip_cmd = [self.python_cmd, "-m", "pip", "install"]
        has_cuda = shutil.which("nvidia-smi") is not None
        py_minor = self._python_minor()

        if has_cuda:
            try:
                result = subprocess.run(
                    ["nvidia-smi"],
                    capture_output=True,
                    text=True,
                )
                if "CUDA Version" in result.stdout:
                    for line in result.stdout.split("\n"):
                        if "CUDA Version" in line:
                            cuda_version = line.split("CUDA Version:")[1].split()[0]
                            self.logger.info(f"CUDA detectado: {cuda_version}")
                            # Python 3.13+: índice cu121 pode não oferecer torchvision compatível;
                            # usar wheels PyPI (torch+cuda alinhado a torchvision).
                            if py_minor >= 13:
                                self.logger.info("Python 3.13+ — torch+torchvision (PyPI, CUDA)...")
                                subprocess.run(pip_cmd + ["torch", "torchvision"], check=True)
                                return
                            if cuda_version.startswith("12"):
                                self.logger.info("PyTorch CUDA 12.1 (índice oficial)...")
                                subprocess.run(
                                    pip_cmd
                                    + [
                                        "torch",
                                        "torchvision",
                                        "--index-url",
                                        "https://download.pytorch.org/whl/cu121",
                                    ],
                                    check=True,
                                )
                            else:
                                self.logger.info("PyTorch CUDA 11.8...")
                                subprocess.run(
                                    pip_cmd
                                    + [
                                        "torch",
                                        "torchvision",
                                        "--index-url",
                                        "https://download.pytorch.org/whl/cu118",
                                    ],
                                    check=True,
                                )
                            return
            except Exception:
                pass

        self.logger.warn("PyTorch CPU...")
        subprocess.run(
            pip_cmd + ["torch", "torchvision", "--index-url", "https://download.pytorch.org/whl/cpu"],
            check=True,
        )

    def setup_models(self) -> None:
        self.logger.step("Modelos Hugging Face...")
        if self.skip_models:
            return

        models_dir = self.script_dir / "models"
        if models_dir.is_dir() and any(models_dir.iterdir()):
            self.logger.info(f"✓ Diretório de modelos local: {models_dir}")
            config_dir = Path.home() / ".config" / "text2d"
            config_dir.mkdir(parents=True, exist_ok=True)
            config_file = config_dir / "config.env"
            with open(config_file, "w", encoding="utf-8") as f:
                f.write("# Text2D\n")
                f.write(f"TEXT2D_MODELS_DIR={models_dir}\n")
                f.write(f"TEXT2D_OUTPUT_DIR={Path.home() / '.text2d' / 'outputs'}\n")
            self.logger.info(f"Config: {config_file}")
        else:
            self.logger.info("Os pesos SDNQ serão descarregados na primeira execução de `text2d generate`")
            self.logger.info(
                "Pré-download opcional: huggingface-cli download "
                "Disty0/FLUX.2-klein-4B-SDNQ-4bit-dynamic"
            )

    def create_wrappers(self) -> None:
        self.logger.step("Wrappers em bin/...")
        bin_dir = self.install_prefix / "bin"
        bin_dir.mkdir(parents=True, exist_ok=True)

        if self.venv_exists and self.use_venv:
            python_path = str(self.venv_python)
        else:
            python_path = self.python_cmd

        wrapper = bin_dir / "text2d"
        with open(wrapper, "w", encoding="utf-8") as f:
            f.write("#!/bin/bash\n")
            f.write("# Text2D — gerado por installer.py\n")
            f.write(f'exec "{python_path}" -m text2d "$@"\n')
        wrapper.chmod(0o755)
        self.logger.info(f"✓ {wrapper}")

        wrapper_gen = bin_dir / "text2d-generate"
        with open(wrapper_gen, "w", encoding="utf-8") as f:
            f.write("#!/bin/bash\n")
            f.write(f'exec "{bin_dir}/text2d" generate "$@"\n')
        wrapper_gen.chmod(0o755)

        if self.venv_exists and self.use_venv:
            wrapper_act = bin_dir / "text2d-activate"
            with open(wrapper_act, "w", encoding="utf-8") as f:
                f.write("#!/bin/bash\n")
                f.write(f'source "{self.venv_dir}/bin/activate"\n')
                f.write('exec "$@"\n')
            wrapper_act.chmod(0o755)
            self.logger.info(f"✓ {wrapper_act}")

    def setup_directories(self) -> None:
        out = Path.home() / ".text2d" / "outputs"
        (out / "images").mkdir(parents=True, exist_ok=True)
        self.logger.info(f"Diretórios de saída: {out}")

    def show_summary(self) -> None:
        print("\n" + "=" * 42)
        print(f"{Colors.GREEN}  Text2D — instalação concluída{Colors.NC}")
        print("=" * 42 + "\n")

        if self.venv_exists and self.use_venv:
            print(f"✓ venv: {self.venv_dir}\n")

        print("Comandos:")
        print("  text2d --help")
        print("  text2d generate \"uma paisagem ao pôr do sol\" -o out.png")
        print("  text2d info")
        print()
        print("  Testes (dev): pip install -e \".[dev]\" && pytest tests/ -v")
        print()
        print(f"  Saída padrão (documentação): ~/.text2d/outputs/")
        print(f"  Binários: {self.install_prefix}/bin/")
        print()

        w = shutil.which("text2d")
        if w:
            print(f"✓ text2d no PATH: {w}")
        else:
            print(f'⚠ Adicione ao PATH: export PATH="{self.install_prefix}/bin:$PATH"')
        print()


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Text2D — instalador (estilo Text3D)",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Exemplos:
  python3 scripts/installer.py --use-venv
  python3 scripts/installer.py --prefix ~/.local
  sudo python3 scripts/installer.py --prefix /usr/local
  python3 scripts/installer.py --use-venv --skip-deps --skip-models --force

Variáveis:
  INSTALL_PREFIX   diretório de instalação (binários)
  PYTHON_CMD       interpretador Python (default: python3)

Python 3.13+ com GPU: PyTorch+CUDA via PyPI (torchvision alinhado).
Python 3.10–3.12 com GPU: índice cu121 ou cu118.
        """,
    )
    parser.add_argument(
        "--prefix",
        default=os.environ.get("INSTALL_PREFIX", str(Path.home() / ".local")),
        help="Prefixo de instalação (default: ~/.local)",
    )
    parser.add_argument("--use-venv", action="store_true", help="Usar .venv na raiz do projeto")
    parser.add_argument("--skip-deps", action="store_true", help="Avisos mínimos de sistema")
    parser.add_argument("--skip-models", action="store_true", help="Não sugerir modelos/config extra")
    parser.add_argument("--force", action="store_true", help="Reinstalar mesmo se já existir")
    parser.add_argument(
        "--python",
        default=os.environ.get("PYTHON_CMD", "python3"),
        help="Comando Python",
    )

    args = parser.parse_args()

    venv_dir = _project_root() / ".venv"
    if venv_dir.is_dir() and not args.use_venv:
        print(f"{Colors.YELLOW}[INFO]{Colors.NC} Venv em {venv_dir} — use --use-venv para instalação rápida\n")

    installer = Text2DInstaller(args)
    sys.exit(0 if installer.run() else 1)


if __name__ == "__main__":
    main()
