#!/usr/bin/env python3
"""
Rigging3D — instalador system-wide.

Usa gamedev_shared.installer.PythonProjectInstaller para a lógica base.
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


class Rigging3DInstaller(PythonProjectInstaller):
    """Instalador específico do Rigging3D (PyTorch/CUDA + extras inference opcionais)."""

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

    def run(self) -> bool:
        if not super().run():
            return False
        self.create_cli_wrappers()
        self.create_activate_wrapper()
        self.setup_directories()
        self.show_summary(
            commands=[
                "rigging3d --help",
                "rigging3d pipeline -i mesh.glb -o rigged.glb",
                "rigging3d skeleton --help",
            ],
            extras=[
                '[dim]Extras inference (venv):[/dim] pip install -e ".[inference]"'
                if self.logger.rich_available
                else 'Extras inference (venv): pip install -e ".[inference]"',
            ],
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
  python3 scripts/installer.py --use-venv
  python3 scripts/installer.py --prefix ~/.local

Variáveis:
  INSTALL_PREFIX   diretório de instalação (binários)
  PYTHON_CMD       interpretador Python (default: python3)

Nota: instala dependências base; o extra ``[inference]`` (UniRig) é opcional e pesado.
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
    parser.add_argument("--skip-deps", action="store_true", help="Avisos mínimos de sistema")
    parser.add_argument("--skip-models", action="store_true", help="Não mostrar dicas extra de ambiente")
    parser.add_argument("--force", action="store_true", help="Reinstalar mesmo se já existir")
    parser.add_argument(
        "--python",
        default=os.environ.get("PYTHON_CMD", "python3"),
        help="Comando Python",
    )

    args = parser.parse_args()

    installer = Rigging3DInstaller(args)
    sys.exit(0 if installer.run() else 1)


if __name__ == "__main__":
    main()
