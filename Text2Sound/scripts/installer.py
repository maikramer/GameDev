#!/usr/bin/env python3
"""
Text2Sound — instalador system-wide.

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
from gamedev_shared.installer.base import default_python_command


class Text2SoundInstaller(PythonProjectInstaller):
    """Instalador específico do Text2Sound (PyTorch + CUDA)."""

    def __init__(self, args: argparse.Namespace) -> None:
        super().__init__(
            project_name="Text2Sound",
            cli_name="text2sound",
            project_root=_project_root,
            install_prefix=Path(args.prefix),
            python_cmd=args.python,
            use_venv=args.use_venv,
            skip_deps=args.skip_deps,
            skip_models=args.skip_models,
            force=args.force,
            skip_pytorch=False,
        )
        self.args = args

    def run(self) -> bool:
        if not super().run():
            return False
        self.create_cli_wrappers(extra_aliases=["text2sound-generate"])
        self.create_activate_wrapper()
        self.setup_directories()
        self.show_summary(
            commands=[
                "text2sound --help",
                'text2sound generate "ocean waves on a beach at sunset"',
                "text2sound presets",
                "text2sound info",
                'text2sound generate --preset battle "epic boss fight"',
                "text2sound batch prompts.txt --format flac",
            ],
            extras=[
                '[dim]Dev:[/dim] pip install -e ".[dev]" && pytest tests/ -v'
                if self.logger.rich_available
                else 'Dev: pip install -e ".[dev]" && pytest tests/ -v',
            ],
        )
        return True

    def setup_directories(self) -> None:
        out = Path.home() / ".text2sound" / "outputs"
        (out / "audio").mkdir(parents=True, exist_ok=True)
        self.logger.info(f"Diretórios de saída: {out}")


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Text2Sound — instalador",
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

Nota: Text2Sound usa PyTorch + torchaudio. CUDA recomendado.
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
        help="Legado (no-op). O instalador cria .venv no projecto se necessário.",
    )
    parser.add_argument("--skip-deps", action="store_true", help="Avisos mínimos de sistema")
    parser.add_argument("--skip-models", action="store_true", help="Não mostrar dicas extra de ambiente")
    parser.add_argument("--force", action="store_true", help="Reinstalar mesmo se já existir")
    parser.add_argument(
        "--python",
        default=default_python_command(),
        help="Comando Python",
    )

    args = parser.parse_args()

    installer = Text2SoundInstaller(args)
    sys.exit(0 if installer.run() else 1)


if __name__ == "__main__":
    main()
