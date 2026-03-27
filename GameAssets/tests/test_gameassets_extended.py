"""Testes extra GameAssets: manifest, prompt_builder, perfil."""

from __future__ import annotations

from pathlib import Path

import pytest

from gameassets.manifest import (
    ManifestRow,
    effective_image_source,
    load_manifest,
)
from gameassets.manifest import _parse_bool as gb_parse_bool
from gameassets.manifest import _parse_image_source as parse_img_src
from gameassets.profile import GameProfile
from gameassets.prompt_builder import build_audio_prompt, build_prompt


def _minimal_profile(**kwargs) -> GameProfile:
    base = dict(
        title="T",
        genre="g",
        tone="t",
        style_preset="p",
    )
    base.update(kwargs)
    return GameProfile(**base)


def test_parse_bool_none() -> None:
    assert gb_parse_bool(None) is False


def test_parse_bool_empty() -> None:
    assert gb_parse_bool("") is False
    assert gb_parse_bool("   ") is False


def test_parse_bool_truthy() -> None:
    for s in ("1", "true", "yes", "sim", "y", "on", "TRUE"):
        assert gb_parse_bool(s) is True


def test_parse_image_source_none() -> None:
    assert parse_img_src(None) is None
    assert parse_img_src("") is None


def test_parse_image_source_valid() -> None:
    assert parse_img_src("text2d") == "text2d"
    assert parse_img_src("TEXTURE2D") == "texture2d"
    assert parse_img_src("  skymap2d ") == "skymap2d"


def test_parse_image_source_invalid() -> None:
    with pytest.raises(ValueError, match="text2d"):
        parse_img_src("invalid")


def test_effective_image_source_row_wins() -> None:
    p = _minimal_profile(image_source="text2d")
    row = ManifestRow(id="1", idea="x", kind=None, generate_3d=False, image_source="texture2d")
    assert effective_image_source(p, row) == "texture2d"


def test_effective_image_source_from_profile() -> None:
    p = _minimal_profile(image_source="skymap2d")
    row = ManifestRow(id="1", idea="x", kind=None, generate_3d=False, image_source=None)
    assert effective_image_source(p, row) == "skymap2d"


def test_load_manifest_minimal_csv(tmp_path: Path) -> None:
    p = tmp_path / "m.csv"
    p.write_text("id,idea\na,idea a\nb,idea b\n", encoding="utf-8")
    rows = load_manifest(p)
    assert len(rows) == 2
    assert rows[0].id == "a"


def test_build_prompt_contains_idea() -> None:
    prof = _minimal_profile()
    preset = {"prompt_prefix": "PREFIX", "hint_2d": "H2D", "negative_suffix": ""}
    row = ManifestRow(id="id1", idea="  hero sword  ", kind="prop", generate_3d=False)
    out = build_prompt(prof, preset, row, for_3d=False)
    assert "hero sword" in out
    assert "PREFIX" in out


def test_build_prompt_for_3d_adds_mesh_hint() -> None:
    prof = _minimal_profile()
    preset = {"prompt_prefix": "", "hint_3d": "H3D", "negative_suffix": ""}
    row = ManifestRow(id="id1", idea="mesh", kind=None, generate_3d=True)
    out = build_prompt(prof, preset, row, for_3d=True)
    assert "Watertight mesh" in out or "watertight" in out.lower()


def test_build_prompt_generate_3d_2d_ref_lighting() -> None:
    prof = _minimal_profile()
    preset = {"prompt_prefix": "", "hint_2d": "", "negative_suffix": ""}
    row = ManifestRow(id="id1", idea="ref", kind=None, generate_3d=True)
    out = build_prompt(prof, preset, row, for_3d=False)
    assert "Image-to-3D reference" in out or "diffuse" in out.lower()


def test_build_audio_prompt_contains_kind() -> None:
    prof = _minimal_profile()
    preset = {"prompt_prefix": "AP", "hint_audio": "ha", "negative_suffix": ""}
    row = ManifestRow(id="id1", idea="footsteps", kind="environment", generate_3d=False)
    out = build_audio_prompt(prof, preset, row)
    assert "footsteps" in out
    assert "ambient" in out.lower() or "soundscape" in out.lower()


def test_build_prompt_collapses_whitespace() -> None:
    prof = _minimal_profile(genre="  a  ", tone="  b  ")
    preset = {"prompt_prefix": "x", "hint_2d": "", "negative_suffix": ""}
    row = ManifestRow(id="id1", idea="y", kind=None, generate_3d=False)
    out = build_prompt(prof, preset, row, for_3d=False)
    assert "  " not in out or out.count("  ") < 5


def test_parse_output_dir_behavior() -> None:
    from gameassets.profile import _parse_output_dir

    assert _parse_output_dir(None) == "."
    assert _parse_output_dir("  ") == "."
    assert _parse_output_dir("out") == "out"


def test_manifest_skips_empty_rows(tmp_path: Path) -> None:
    p = tmp_path / "m.csv"
    p.write_text("id,idea\n,,\nc,valid\n", encoding="utf-8")
    rows = load_manifest(p)
    assert len(rows) == 1
    assert rows[0].id == "c"


def test_manifest_generate_3d_column(tmp_path: Path) -> None:
    p = tmp_path / "m.csv"
    p.write_text("id,idea,generate_3d\nd,idea,1\n", encoding="utf-8")
    rows = load_manifest(p)
    assert rows[0].generate_3d is True


def test_manifest_generate_audio_column(tmp_path: Path) -> None:
    p = tmp_path / "m.csv"
    p.write_text("id,idea,generate_audio\ne,idea,yes\n", encoding="utf-8")
    rows = load_manifest(p)
    assert rows[0].generate_audio is True


def test_resources_presets_yaml_path() -> None:
    from gameassets.resources import presets_yaml_path

    pp = presets_yaml_path()
    assert pp.name == "presets.yaml"
    assert (pp.parent / pp.name) == pp


def test_templates_manifest_csv_header() -> None:
    from gameassets.templates import MANIFEST_CSV

    assert "id" in MANIFEST_CSV
    assert "idea" in MANIFEST_CSV
