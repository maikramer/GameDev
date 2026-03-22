"""Instala a Agent Skill Cursor: origem GameDev/.cursor/create-skill ou cursor_skill/."""

from __future__ import annotations

import shutil
from pathlib import Path

TOOL_NAME = "text2d"


def _package_dir() -> Path:
    return Path(__file__).resolve().parent


def _editable_repo_root() -> Path | None:
    """Text2D/src/text2d -> Text2D."""
    p = _package_dir()
    if p.parent.name == "src":
        return p.parent.parent
    return None


def resolve_skill_source() -> Path:
    """Prefer monorepo .cursor/create-skill; fallback package cursor_skill/."""
    repo = _editable_repo_root()
    if repo is not None:
        gamedev = repo.parent
        cand = gamedev / ".cursor" / "create-skill" / TOOL_NAME
        if cand.is_dir() and (cand / "SKILL.md").is_file():
            return cand
    vend = _package_dir() / "cursor_skill"
    if (vend / "SKILL.md").is_file():
        return vend
    raise FileNotFoundError(
        f"Skill {TOOL_NAME} não encontrada: esperado "
        f"GameDev/.cursor/create-skill/{TOOL_NAME}/SKILL.md ou "
        f"text2d/cursor_skill/SKILL.md."
    )


def install_agent_skill(target_root: Path, *, force: bool = False) -> Path:
    """Copia SKILL.md para target_root/.cursor/skills/<TOOL_NAME>/SKILL.md."""
    src = resolve_skill_source()
    target_root = target_root.resolve()
    dest_dir = target_root / ".cursor" / "skills" / TOOL_NAME
    dest_dir.mkdir(parents=True, exist_ok=True)
    dest_file = dest_dir / "SKILL.md"
    if dest_file.exists() and not force:
        raise FileExistsError(str(dest_file))
    shutil.copy2(src / "SKILL.md", dest_file)
    return dest_file
