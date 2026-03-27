"""Instalação genérica de Agent Skills Cursor para ferramentas do monorepo."""

from __future__ import annotations

import shutil
from pathlib import Path

from .installer.registry import try_find_monorepo_root


def resolve_skill_source(
    tool_name: str,
    package_dir: Path,
) -> Path:
    """Localiza a skill source: monorepo .cursor/create-skill ou cursor_skill/ no pacote.

    Args:
        tool_name: Nome da ferramenta (ex: ``"text2d"``, ``"text3d"``, ``"gameassets"``).
        package_dir: Directório do pacote Python (``Path(__file__).resolve().parent``
                     chamado a partir do módulo da ferramenta).

    Returns:
        Path para o directório que contém ``SKILL.md``.

    Raises:
        FileNotFoundError: Skill não encontrada em nenhuma localização.
    """
    gamedev = try_find_monorepo_root(package_dir)
    if gamedev is not None:
        cand = gamedev / ".cursor" / "create-skill" / tool_name
        if cand.is_dir() and (cand / "SKILL.md").is_file():
            return cand

    vend = package_dir / "cursor_skill"
    if (vend / "SKILL.md").is_file():
        return vend

    raise FileNotFoundError(
        f"Skill {tool_name} não encontrada: esperado "
        f"GameDev/.cursor/create-skill/{tool_name}/SKILL.md ou "
        f"{package_dir.name}/cursor_skill/SKILL.md."
    )


def install_agent_skill(
    tool_name: str,
    package_dir: Path,
    target_root: Path,
    *,
    force: bool = False,
) -> Path:
    """Copia SKILL.md para target_root/.cursor/skills/<tool_name>/SKILL.md.

    Args:
        tool_name: Nome da ferramenta.
        package_dir: Directório do pacote Python da ferramenta.
        target_root: Raiz do projecto destino.
        force: Sobrescrever se já existir.

    Returns:
        Caminho do ficheiro copiado.

    Raises:
        FileExistsError: Skill já existe e ``force`` é False.
        FileNotFoundError: Source skill não encontrada.
    """
    src = resolve_skill_source(tool_name, package_dir)
    target_root = target_root.resolve()
    dest_dir = target_root / ".cursor" / "skills" / tool_name
    dest_dir.mkdir(parents=True, exist_ok=True)
    dest_file = dest_dir / "SKILL.md"
    if dest_file.exists() and not force:
        raise FileExistsError(str(dest_file))
    shutil.copy2(src / "SKILL.md", dest_file)
    return dest_file
