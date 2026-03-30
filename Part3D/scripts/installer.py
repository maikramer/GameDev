#!/usr/bin/env python3
"""
Part3D — wrapper do instalador oficial (``gamedev-install part3d`` / ``./install.sh part3d``).

A lógica vive em ``gamedev_shared.installer.part3d_extras``.
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
from gamedev_shared.installer.part3d_extras import run_part3d_post_install


class Part3DInstaller(PythonProjectInstaller):
    """Instalador Part3D (delega pós-venv a :func:`run_part3d_post_install`)."""

    def __init__(self, args: argparse.Namespace) -> None:
        super().__init__(
            project_name="Part3D",
            cli_name="part3d",
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
        return run_part3d_post_install(self)

    def check_python(self, min_version: tuple[int, int] = (3, 10)) -> bool:
        return super().check_python(min_version)


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Part3D — instalador (equivalente a ./install.sh part3d)",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Forma oficial no monorepo: ./install.sh part3d  (ou: gamedev-install part3d)

Exemplos:
  ./install.sh part3d
  python3 scripts/installer.py --force
  python3 scripts/installer.py --skip-deps

Variáveis:
  INSTALL_PREFIX    Diretório de instalação
  PYTHON_CMD        Interpretador Python
        """,
    )
    parser.add_argument(
        "--prefix",
        default=os.environ.get("INSTALL_PREFIX", str(Path.home() / ".local")),
        help="Diretório de instalação (padrão: ~/.local)",
    )
    parser.add_argument(
        "--use-venv",
        action="store_true",
        help="Legado (no-op). O instalador cria .venv no projecto se necessário.",
    )
    parser.add_argument("--skip-deps", action="store_true", help="Pular verificação de deps do sistema")
    parser.add_argument("--skip-models", action="store_true", help="Pular configuração de modelos")
    parser.add_argument("--force", action="store_true", help="Forçar reinstalação")
    parser.add_argument(
        "--python",
        default=default_python_command(),
        help="Comando Python",
    )

    args = parser.parse_args()

    installer = Part3DInstaller(args)
    sys.exit(0 if installer.run() else 1)


if __name__ == "__main__":
    main()
