#!/usr/bin/env python3
"""
Text2D — instalador system-wide.

Usa gamedev_shared.installer.PythonProjectInstaller para a lógica base.
"""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

# Permite execução directa sem pip install do Shared.
_script_dir = Path(__file__).resolve().parent
_project_root = _script_dir.parent
_shared_src = _project_root.parent / "Shared" / "src"
if _shared_src.is_dir() and str(_shared_src) not in sys.path:
    sys.path.insert(0, str(_shared_src))

from gamedev_shared.installer import PythonProjectInstaller
from gamedev_shared.installer.base import default_python_command


class Text2DInstaller(PythonProjectInstaller):
    """Instalador específico do Text2D."""

    def __init__(self, args: argparse.Namespace) -> None:
        super().__init__(
            project_name="Text2D",
            cli_name="text2d",
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
        self.setup_models()
        self.create_cli_wrappers(extra_aliases=["text2d-generate"])
        self.create_activate_wrapper()
        self.setup_directories()
        self.show_summary(
            commands=[
                "text2d --help",
                'text2d generate "uma paisagem ao pôr do sol" -o out.png',
                "text2d info",
            ],
            extras=[
                '[dim]Dev:[/dim] pip install -e ".[dev]" && pytest tests/ -v'
                if self.logger.rich_available
                else 'Dev: pip install -e ".[dev]" && pytest tests/ -v',
            ],
        )
        return True

    def setup_models(self) -> None:
        self.logger.step("Modelos Hugging Face...")
        if self.skip_models:
            return

        models_dir = self.project_root / "models"
        if models_dir.is_dir() and any(models_dir.iterdir()):
            self.logger.info(f"Diretório de modelos local: {models_dir}")
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
            self.logger.info("Pré-download opcional: huggingface-cli download Disty0/FLUX.2-klein-4B-SDNQ-4bit-dynamic")

    def setup_directories(self) -> None:
        out = Path.home() / ".text2d" / "outputs"
        (out / "images").mkdir(parents=True, exist_ok=True)
        self.logger.info(f"Diretórios de saída: {out}")


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Text2D — instalador",
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
    parser.add_argument("--skip-models", action="store_true", help="Não sugerir modelos/config extra")
    parser.add_argument("--force", action="store_true", help="Reinstalar mesmo se já existir")
    parser.add_argument(
        "--python",
        default=default_python_command(),
        help="Comando Python",
    )

    args = parser.parse_args()

    installer = Text2DInstaller(args)
    sys.exit(0 if installer.run() else 1)


if __name__ == "__main__":
    main()
