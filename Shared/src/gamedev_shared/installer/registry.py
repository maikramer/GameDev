"""Registry de ferramentas do monorepo GameDev.

Define metadata (tipo, caminhos, nomes) para cada ferramenta instalável,
permitindo que um CLI unificado descubra e instale qualquer uma delas.
"""

from __future__ import annotations

import platform
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import Optional


class ToolKind(Enum):
    PYTHON = "python"
    RUST = "rust"


@dataclass(frozen=True)
class ToolSpec:
    """Especificação de uma ferramenta instalável."""

    name: str
    kind: ToolKind
    folder: str
    cli_name: str
    description: str

    cargo_bin_name: str = ""
    python_module: str = ""
    min_python: tuple[int, int] = (3, 10)
    extra_aliases: tuple[str, ...] = ()
    needs_pytorch: bool = False
    needs_cuda: bool = False

    def project_root(self, monorepo: Path) -> Path:
        return monorepo / self.folder

    def exists(self, monorepo: Path) -> bool:
        root = self.project_root(monorepo)
        if self.kind == ToolKind.PYTHON:
            return (root / "setup.py").is_file() or (root / "pyproject.toml").is_file()
        if self.kind == ToolKind.RUST:
            return (root / "Cargo.toml").is_file()
        return root.is_dir()


TOOLS: dict[str, ToolSpec] = {
    "text2d": ToolSpec(
        name="Text2D",
        kind=ToolKind.PYTHON,
        folder="Text2D",
        cli_name="text2d",
        python_module="text2d",
        description="CLI text-to-image com FLUX.2 Klein (SDNQ)",
        min_python=(3, 10),
        extra_aliases=("text2d-generate",),
        needs_pytorch=True,
        needs_cuda=True,
    ),
    "text3d": ToolSpec(
        name="Text3D",
        kind=ToolKind.PYTHON,
        folder="Text3D",
        cli_name="text3d",
        python_module="text3d",
        description="Pipeline text-to-3D (Text2D + Hunyuan3D)",
        min_python=(3, 8),
        extra_aliases=("text3d-generate",),
        needs_pytorch=True,
        needs_cuda=True,
    ),
    "gameassets": ToolSpec(
        name="GameAssets",
        kind=ToolKind.PYTHON,
        folder="GameAssets",
        cli_name="gameassets",
        python_module="gameassets",
        description="Batch de prompts/assets 2D/3D alinhados ao jogo",
        min_python=(3, 10),
        needs_pytorch=False,
    ),
    "text2sound": ToolSpec(
        name="Text2Sound",
        kind=ToolKind.PYTHON,
        folder="Text2Sound",
        cli_name="text2sound",
        python_module="text2sound",
        description="CLI text-to-audio com Stable Audio Open 1.0",
        min_python=(3, 10),
        extra_aliases=("text2sound-generate",),
        needs_pytorch=True,
        needs_cuda=True,
    ),
    "texture2d": ToolSpec(
        name="Texture2D",
        kind=ToolKind.PYTHON,
        folder="Texture2D",
        cli_name="texture2d",
        python_module="texture2d",
        description="Texturas 2D seamless via HF Inference API (sem GPU local)",
        min_python=(3, 10),
        extra_aliases=("texture2d-generate",),
        needs_pytorch=False,
    ),
    "skymap2d": ToolSpec(
        name="Skymap2D",
        kind=ToolKind.PYTHON,
        folder="Skymap2D",
        cli_name="skymap2d",
        python_module="skymap2d",
        description="Skymaps equirectangular 360° via HF Inference API (sem GPU local)",
        min_python=(3, 10),
        extra_aliases=("skymap2d-generate",),
        needs_pytorch=False,
    ),
    "materialize": ToolSpec(
        name="Materialize CLI",
        kind=ToolKind.RUST,
        folder="Materialize",
        cli_name="materialize",
        cargo_bin_name="materialize-cli",
        description="PBR maps (normal, AO, metallic, smoothness) via GPU/wgpu",
    ),
}


def find_monorepo_root(start: Path | None = None) -> Path:
    """Encontra a raiz do monorepo GameDev navegando para cima.

    Procura pasta com ``.git`` + ``Shared/``.
    """
    if start is None:
        start = Path(__file__).resolve()
    current = start
    for _ in range(10):
        if (current / ".git").exists() and (current / "Shared").is_dir():
            return current
        parent = current.parent
        if parent == current:
            break
        current = parent
    raise FileNotFoundError(
        "Raiz do monorepo GameDev não encontrada. "
        "Execute a partir de dentro do repositório."
    )


def list_available_tools(monorepo: Path | None = None) -> list[ToolSpec]:
    """Lista ferramentas cujo directório existe no monorepo."""
    if monorepo is None:
        monorepo = find_monorepo_root()
    return [spec for spec in TOOLS.values() if spec.exists(monorepo)]


def get_tool(name: str) -> ToolSpec:
    """Retorna spec de uma ferramenta pelo nome (case-insensitive).

    Raises:
        KeyError: Ferramenta não conhecida.
    """
    key = name.lower().replace("-", "").replace("_", "")
    for k, spec in TOOLS.items():
        if k == key or spec.cli_name == key or spec.name.lower().replace(" ", "") == key:
            return spec
    raise KeyError(
        f"Ferramenta desconhecida: {name!r}. "
        f"Disponíveis: {', '.join(TOOLS.keys())}"
    )
