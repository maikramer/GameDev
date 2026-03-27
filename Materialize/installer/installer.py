#!/usr/bin/env python3
"""
Materialize CLI Installer - Cross-platform installation.

Usa gamedev_shared.installer.RustProjectInstaller para a lógica base.
"""

import sys
from pathlib import Path

# O instalador pode ser executado sem pip install do Shared.
# Adiciona o Shared ao sys.path se não estiver instalado como pacote.
_installer_dir = Path(__file__).resolve().parent
_repo_dir = _installer_dir.parent
_shared_src = _repo_dir.parent / "Shared" / "src"
if _shared_src.is_dir() and str(_shared_src) not in sys.path:
    sys.path.insert(0, str(_shared_src))

from gamedev_shared.installer import RustProjectInstaller

CARGO_BIN_NAME = "materialize-cli"
CLI_NAME = "materialize"


class MaterializeInstaller(RustProjectInstaller):
    """Instalador específico do Materialize CLI."""

    def __init__(self) -> None:
        super().__init__(
            project_name="Materialize CLI",
            cli_name=CLI_NAME,
            project_root=_repo_dir,
            cargo_bin_name=CARGO_BIN_NAME,
        )


def main():
    import argparse

    parser = argparse.ArgumentParser(
        description="Materialize CLI Installer (Rust)",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  ./install.sh              # Install (Linux/macOS)
  python3 installer/installer.py install
  python3 installer/installer.py uninstall
  python3 installer/installer.py reinstall
        """,
    )
    parser.add_argument(
        "action",
        nargs="?",
        default="install",
        choices=["install", "uninstall", "reinstall"],
        help="Action (default: install)",
    )
    args = parser.parse_args()

    installer = MaterializeInstaller()

    if args.action == "install":
        success = installer.run()
    elif args.action == "uninstall":
        success = installer.run_uninstall()
    elif args.action == "reinstall":
        success = installer.run_reinstall()
    else:
        success = False

    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()
