#!/usr/bin/env python3
"""
Text3D — wrapper do instalador oficial (``gamedev-install text3d`` / ``./install.sh text3d``).

A lógica vive em ``gamedev_shared.installer``; este script apenas expõe a mesma interface CLI.
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
from gamedev_shared.installer.text3d_extras import Text3DPostInstall


class Text3DInstaller(PythonProjectInstaller):
    """Instalador específico do Text3D (delega pós-venv a :class:`Text3DPostInstall`)."""

    def __init__(self, args: argparse.Namespace) -> None:
        super().__init__(
            project_name="Text3D",
            cli_name="text3d",
            project_root=_project_root,
            install_prefix=Path(args.prefix),
            python_cmd=args.python,
            use_venv=args.use_venv,
            skip_deps=args.skip_deps,
            skip_models=args.skip_models,
            force=args.force,
        )
        self.args = args
        self.skip_env_config = args.skip_env_config

    def run(self) -> bool:
        if not super().run():
            return False
        Text3DPostInstall(self, skip_env_config=self.skip_env_config).run()
        return True

    def check_python(self, min_version: tuple[int, int] = (3, 10)) -> bool:
        return super().check_python(min_version)

    def install_in_venv(self) -> None:
        text2d = self.project_root.parent / "Text2D"
        if not text2d.is_dir():
            self.logger.warn("Monorepo: espera-se Text2D ao lado de Text3D (ex.: GameDev/Text2D + GameDev/Text3D).")
        super().install_in_venv()


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Text3D — instalador (equivalente a ./install.sh text3d)",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Forma oficial no monorepo: ./install.sh text3d  (ou: gamedev-install text3d)

Exemplos:
  ./install.sh text3d
  python3 scripts/installer.py --prefix ~/.local
  python3 scripts/installer.py --skip-env-config

Variáveis:
  INSTALL_PREFIX    Diretório de instalação
  PYTHON_CMD        Interpretador Python (defeito: python no Windows, python3 no Linux/macOS)
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
        "--skip-env-config",
        action="store_true",
        help="Não escrever ~/.config/text3d/env.sh",
    )
    parser.add_argument(
        "--python",
        default=default_python_command(),
        help="Comando Python (defeito: python no Windows, python3 noutros)",
    )

    args = parser.parse_args()

    installer = Text3DInstaller(args)
    sys.exit(0 if installer.run() else 1)


if __name__ == "__main__":
    main()
