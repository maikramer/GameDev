#!/usr/bin/env python3
"""
Text3D — instalador system-wide.

Usa gamedev_shared.installer.PythonProjectInstaller para a lógica base.
"""

from __future__ import annotations

import argparse
import os
import shutil
import sys
from pathlib import Path

_script_dir = Path(__file__).resolve().parent
_project_root = _script_dir.parent
_shared_src = _project_root.parent / "Shared" / "src"
if _shared_src.is_dir() and str(_shared_src) not in sys.path:
    sys.path.insert(0, str(_shared_src))

from gamedev_shared.installer import PythonProjectInstaller


class Text3DInstaller(PythonProjectInstaller):
    """Instalador específico do Text3D."""

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

        self.setup_models()
        self.create_text3d_wrappers()
        self.setup_directories()

        if not self.skip_env_config:
            self.write_env_file()

        self._show_text3d_summary()
        return True

    def check_python(self, min_version: tuple[int, int] = (3, 8)) -> bool:
        return super().check_python(min_version)

    def _warn_monorepo_layout(self) -> None:
        text2d = self.project_root.parent / "Text2D"
        if not text2d.is_dir():
            self.logger.warn(
                "Monorepo: espera-se Text2D ao lado de Text3D (ex.: GameDev/Text2D + GameDev/Text3D)."
            )

    def install_in_venv(self) -> None:
        self._warn_monorepo_layout()
        super().install_in_venv()

    def install_system_wide(self) -> None:
        self._warn_monorepo_layout()
        super().install_system_wide()

    def setup_models(self) -> None:
        self.logger.step("Configurando modelos...")

        hf_cache = Path.home() / ".cache" / "huggingface"
        models_dir = self.project_root / "models"

        if not self.skip_models:
            self.logger.info(
                "Text2D (FLUX/SDNQ) e Hunyuan3D-2mini vêm do Hugging Face na primeira execução."
            )
            self.logger.info(f"Cache típico: {hf_cache}")
            self.logger.info("Opcional: huggingface-cli login (modelos gated / quotas)")

        if models_dir.exists() and any(models_dir.iterdir()):
            self.logger.info(f"Pasta local opcional (assets): {models_dir}")

        config_dir = Path.home() / ".config" / "text3d"
        config_dir.mkdir(parents=True, exist_ok=True)
        config_file = config_dir / "config.env"
        if not config_file.exists():
            with open(config_file, "w", encoding="utf-8") as f:
                f.write("# Text3D — gerado por scripts/installer.py\n")
                f.write(f"TEXT3D_OUTPUT_DIR={Path.home() / '.text3d/outputs'}\n")
                if models_dir.exists():
                    f.write(f"TEXT3D_MODELS_DIR={models_dir}\n")
            self.logger.info(f"Config criada: {config_file}")
        else:
            self.logger.info(f"Config existente (mantida): {config_file}")

    def write_env_file(self) -> None:
        config_dir = Path.home() / ".config" / "text3d"
        config_dir.mkdir(parents=True, exist_ok=True)
        env_sh = config_dir / "env.sh"
        content = (
            "# Text3D — gerado por scripts/installer.py\n"
            "# source ~/.config/text3d/env.sh\n"
            'export PYTORCH_CUDA_ALLOC_CONF="${PYTORCH_CUDA_ALLOC_CONF:-expandable_segments:True}"\n'
            "\n"
            "# Descomenta se compilaste custom_rasterizer com um toolkit específico:\n"
            "# export CUDA_HOME=/usr/local/cuda-11.8\n"
        )
        with open(env_sh, "w", encoding="utf-8") as f:
            f.write(content)
        self.logger.info(f"Ambiente opcional: {env_sh}")

    def create_text3d_wrappers(self) -> None:
        python_path = (
            str(self.venv_python) if (self.venv_exists and self.use_venv) else self.python_cmd
        )
        env_sh = Path.home() / ".config" / "text3d" / "env.sh"

        self.bin_dir.mkdir(parents=True, exist_ok=True)

        wrapper = self.bin_dir / "text3d"
        with open(wrapper, "w", encoding="utf-8") as f:
            f.write("#!/bin/bash\n")
            f.write("# Text3D wrapper — gerado por installer\n")
            f.write(f'if [[ -f "{env_sh}" ]]; then\n')
            f.write('  # shellcheck source=/dev/null\n')
            f.write(f'  . "{env_sh}"\n')
            f.write("fi\n")
            f.write(f'exec "{python_path}" -m text3d "$@"\n')
        wrapper.chmod(0o755)
        self.logger.success(str(wrapper))

        wrapper_gen = self.bin_dir / "text3d-generate"
        with open(wrapper_gen, "w", encoding="utf-8") as f:
            f.write("#!/bin/bash\n")
            f.write(f'exec "{self.bin_dir}/text3d" generate "$@"\n')
        wrapper_gen.chmod(0o755)

        self.create_activate_wrapper()

    def setup_directories(self) -> None:
        output_dir = Path.home() / ".text3d" / "outputs"
        (output_dir / "meshes").mkdir(parents=True, exist_ok=True)
        (output_dir / "gifs").mkdir(parents=True, exist_ok=True)
        (output_dir / "images").mkdir(parents=True, exist_ok=True)
        self.logger.info(f"Diretórios de saída: {output_dir}")

    def _show_text3d_summary(self) -> None:
        self.show_summary(
            commands=[
                "text3d doctor",
                "text3d --help",
                "text3d generate 'um robô' -o robô.glb --preset balanced",
                "text3d generate 'prompt' --final -o modelo.glb",
                "text3d texture mesh.glb -i ref.png -o pintado.glb",
            ],
            extras=[
                "[dim]Modelos: cache ~/.cache/huggingface[/dim]"
                if self.logger.rich_available
                else "Modelos: cache ~/.cache/huggingface",
            ],
        )


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Text3D — instalador",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Exemplos:
  python3 scripts/installer.py --use-venv
  python3 scripts/installer.py --prefix ~/.local
  sudo python3 scripts/installer.py --prefix /usr/local
  python3 scripts/installer.py --use-venv --skip-deps --skip-models

Variáveis:
  INSTALL_PREFIX    Diretório de instalação
  PYTHON_CMD        Interpretador Python (default: python3)
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
        default=os.environ.get("PYTHON_CMD", "python3"),
        help="Comando Python (padrão: python3)",
    )

    args = parser.parse_args()

    installer = Text3DInstaller(args)
    sys.exit(0 if installer.run() else 1)


if __name__ == "__main__":
    main()
