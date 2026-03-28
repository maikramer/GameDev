#!/usr/bin/env python3
"""
Rigging3D — instalador system-wide.

Usa gamedev_shared.installer.PythonProjectInstaller para a lógica base.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path

_script_dir = Path(__file__).resolve().parent
_project_root = _script_dir.parent
_shared_src = _project_root.parent / "Shared" / "src"
if _shared_src.is_dir() and str(_shared_src) not in sys.path:
    sys.path.insert(0, str(_shared_src))

from gamedev_shared.installer import PythonProjectInstaller


class Rigging3DInstaller(PythonProjectInstaller):
    """Instalador do Rigging3D.

    Fluxo completo (--inference):
      1. venv + pip/setuptools
      2. PyTorch + CUDA
      3. pip install -e ".[inference]"
      4. spconv + cumm (versão auto-detectada)
      5. torch-scatter + torch-cluster
      6. flash-attn (via script, opcional)
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
        self.skip_flash = getattr(args, "skip_flash", False)

    def run(self) -> bool:
        if not super().run():
            return False

        if self.with_inference:
            self._install_inference_extras()

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
                '[dim]Ou: python scripts/installer.py --inference[/dim]'
                if self.logger.rich_available
                else "Ou: python scripts/installer.py --inference"
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

    def _install_inference_extras(self) -> None:
        """Instala extras de inferência e deps CUDA-specific."""
        python = str(self.venv_python)
        pip_cmd = [python, "-m", "pip", "install"]

        self.logger.step("Instalando rigging3d[inference]...")
        subprocess.run(
            [*pip_cmd, "-e", f"{self.project_root}[inference]"],
            check=True,
            cwd=str(self.project_root),
        )

        self.logger.step("Detectando torch/CUDA para deps nativas...")
        try:
            info = subprocess.run(
                [python, "-c", _TORCH_INFO_SCRIPT],
                capture_output=True, text=True, check=True,
            )
            torch_short, cuda_tag = info.stdout.strip().split()
        except (subprocess.CalledProcessError, ValueError):
            self.logger.warn("Não foi possível detectar torch/CUDA — deps nativas não instaladas")
            return

        self.logger.info(f"torch={torch_short}  cuda_tag={cuda_tag}")

        scatter_url = f"https://data.pyg.org/whl/torch-{torch_short}+{cuda_tag}.html"
        self.logger.info("Instalando torch-scatter, torch-cluster...")
        subprocess.run(
            [*pip_cmd, "torch-scatter", "torch-cluster", "-f", scatter_url],
            cwd=str(self.project_root),
        )

        spconv_pkg = _SPCONV_MAP.get(cuda_tag)
        if spconv_pkg:
            self.logger.info(f"Instalando {spconv_pkg}, {spconv_pkg.replace('spconv', 'cumm')}...")
            cumm_pkg = spconv_pkg.replace("spconv", "cumm")
            subprocess.run([*pip_cmd, cumm_pkg, spconv_pkg], cwd=str(self.project_root))
        else:
            self.logger.warn(f"Sem pacote spconv para {cuda_tag} — instala manualmente")

        if not self.skip_flash:
            flash_script = self.project_root / "scripts" / "install_flash_attn.sh"
            if flash_script.is_file():
                self.logger.step("Instalando flash-attn...")
                pip_bin = str(self.venv_python).replace("/python", "/pip")
                subprocess.run(["bash", str(flash_script), "--pip", pip_bin])
            else:
                self.logger.warn("Script install_flash_attn.sh em falta")

    def setup_directories(self) -> None:
        out = Path.home() / ".rigging3d" / "outputs"
        out.mkdir(parents=True, exist_ok=True)
        self.logger.info(f"Diretório de saída sugerido: {out}")


_TORCH_INFO_SCRIPT = """
import torch, sys
v = torch.__version__.split('+')[0]
parts = v.split('.')
short = f'{parts[0]}.{parts[1]}.0'
c = torch.version.cuda or ''
if c:
    p = c.split('.')
    tag = f'cu{p[0]}{p[1]}'
else:
    tag = 'cpu'
print(f'{short} {tag}')
"""

_SPCONV_MAP = {
    "cu130": "spconv-cu121",
    "cu128": "spconv-cu121",
    "cu126": "spconv-cu121",
    "cu124": "spconv-cu121",
    "cu121": "spconv-cu121",
    "cu118": "spconv-cu118",
}


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Rigging3D — instalador",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Exemplos:
  python3 scripts/installer.py --use-venv                  # só CLI base
  python3 scripts/installer.py --use-venv --inference       # CLI + inferência completa
  python3 scripts/installer.py --inference --skip-flash     # sem flash-attn
  bash scripts/setup.sh                                     # alternativa bash (recomendada)

Variáveis:
  INSTALL_PREFIX   diretório de instalação (binários)
  PYTHON_CMD       interpretador Python (default: python3)

Sem --inference instala apenas CLI base. Com --inference instala PyTorch, bpy, spconv, flash-attn, etc.
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
    parser.add_argument("--inference", action="store_true", help="Instalar extras inference + deps CUDA (spconv, flash-attn, etc.)")
    parser.add_argument("--skip-flash", action="store_true", help="Com --inference, não instalar flash-attn")
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
