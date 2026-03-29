#!/usr/bin/env python3
"""
Rigging3D — instalador system-wide.

Usa gamedev_shared.installer.PythonProjectInstaller para a lógica base e
gamedev_shared.installer.rigging_inference para extras UniRig (spconv, PyG, etc.).
"""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

_script_dir = Path(__file__).resolve().parent
_project_root = _script_dir.parent
_shared_src = _project_root.parent / "Shared" / "src"
if _shared_src.is_dir() and str(_shared_src) not in sys.path:
    sys.path.insert(0, str(_shared_src))

from gamedev_shared.installer import PythonProjectInstaller
from gamedev_shared.installer.base import default_python_command
from gamedev_shared.installer.rigging_inference import install_rigging_inference_extras


class Rigging3DInstaller(PythonProjectInstaller):
    """Instalador do Rigging3D.

    Fluxo completo (--inference):
      1. venv + pip/setuptools
      2. PyTorch + CUDA (``install_pytorch``; ver ``RIGGING3D_FORCE_CUDA`` se NVML falhar)
      3. ``pip install -e .[inference]`` + ensure_cuda_torch + spconv + torch-scatter/cluster

    Variáveis de ambiente:
      RIGGING3D_FORCE_CUDA=1 — força reinstalação torch com CUDA mesmo sem nvidia-smi útil
      RIGGING3D_PYTORCH_CUDA_INDEX — URL do índice PyTorch (default cu130)
    """

    def __init__(self, args: argparse.Namespace) -> None:
        super().__init__(
            project_name="Rigging3D",
            cli_name="rigging3d",
            project_root=_project_root,
            install_prefix=Path(args.prefix),
            python_cmd=args.python,
            use_venv=args.use_venv,
            skip_deps=args.skip_deps,
            skip_models=args.skip_models,
            force=args.force,
        )
        self.args = args
        self.with_inference = getattr(args, "inference", False)

    def check_python(self) -> bool:
        return super().check_python(min_version=(3, 11))

    def run(self) -> bool:
        if not super().run():
            return False

        if self.with_inference:
            if not install_rigging_inference_extras(
                venv_python=self.venv_python,
                project_root=self.project_root,
                logger=self.logger,
            ):
                return False

        self.create_cli_wrappers()
        self.create_activate_wrapper()
        self.setup_directories()

        extras_lines = []
        if not self.with_inference:
            extras_lines.append(
                '[dim]Setup completo (inferência):[/dim] bash scripts/setup.sh'
                if self.logger.rich_available
                else "Setup completo (inferência): bash scripts/setup.sh"
            )
            extras_lines.append(
                '[dim]Ou: python scripts/installer.py --use-venv --inference[/dim]'
                if self.logger.rich_available
                else "Ou: python scripts/installer.py --use-venv --inference"
            )
        else:
            extras_lines.append(
                '[dim]GPU/NVML:[/dim] se torch ficou em CPU, RIGGING3D_FORCE_CUDA=1 ou bash scripts/setup.sh'
                if self.logger.rich_available
                else "GPU/NVML: RIGGING3D_FORCE_CUDA=1 ou bash scripts/setup.sh"
            )
        self.show_summary(
            commands=[
                "rigging3d --help",
                "rigging3d pipeline -i mesh.glb -o rigged.glb",
                "rigging3d skeleton --help",
            ],
            extras=extras_lines or None,
        )
        return True

    def setup_directories(self) -> None:
        out = Path.home() / ".rigging3d" / "outputs"
        out.mkdir(parents=True, exist_ok=True)
        self.logger.info(f"Diretório de saída sugerido: {out}")


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Rigging3D — instalador",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Exemplos:
  python3 scripts/installer.py --use-venv                  # só CLI base
  python3 scripts/installer.py --use-venv --inference       # CLI + inferência completa
  bash scripts/setup.sh                                     # alternativa bash (recomendada)

Variáveis:
  INSTALL_PREFIX              diretório de instalação (binários)
  PYTHON_CMD                  interpretador Python (default: python3)
  RIGGING3D_FORCE_CUDA=1      forçar torch CUDA (p.ex. NVML/driver estragado)
  RIGGING3D_PYTORCH_CUDA_INDEX  URL extra-index para torch (default: cu130)

Sem --inference instala apenas CLI base. Com --inference: PyTorch CUDA, bpy, Open3D,
spconv, torch-scatter/cluster.
        """,
    )
    parser.add_argument(
        "--prefix",
        default=os.environ.get("INSTALL_PREFIX", str(Path.home() / ".local")),
        help="Prefixo de instalação (default: ~/.local)",
    )
    parser.add_argument(
        "--use-venv",
        action="store_true",
        help="Cria .venv no projecto se necessário.",
    )
    parser.add_argument("--inference", action="store_true", help="Instalar extras inference + deps CUDA (spconv, PyG, etc.)")
    parser.add_argument("--skip-deps", action="store_true", help="Avisos mínimos de sistema")
    parser.add_argument("--skip-models", action="store_true", help="Não mostrar dicas extra de ambiente")
    parser.add_argument("--force", action="store_true", help="Reinstalar mesmo se já existir")
    parser.add_argument(
        "--python",
        default=default_python_command(),
        help="Comando Python (defeito: python no Windows)",
    )

    args = parser.parse_args()

    installer = Rigging3DInstaller(args)
    sys.exit(0 if installer.run() else 1)


if __name__ == "__main__":
    main()
