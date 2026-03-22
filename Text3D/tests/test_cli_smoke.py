"""Smoke tests do CLI Text3D (sem carregar Hunyuan / modelos)."""

from __future__ import annotations

from click.testing import CliRunner

from text3d.cli import cli


def test_root_help() -> None:
    runner = CliRunner()
    r = runner.invoke(cli, ["--help"])
    assert r.exit_code == 0
    assert "Text3D" in r.output or "mesh" in r.output.lower()


def test_version() -> None:
    runner = CliRunner()
    r = runner.invoke(cli, ["--version"])
    assert r.exit_code == 0
    assert "0.1.0" in r.output


def test_generate_help() -> None:
    runner = CliRunner()
    r = runner.invoke(cli, ["generate", "--help"])
    assert r.exit_code == 0
    assert "--from-image" in r.output or "from-image" in r.output


def test_generate_requires_prompt_or_image() -> None:
    runner = CliRunner()
    r = runner.invoke(cli, ["generate"])
    assert r.exit_code != 0


def test_info() -> None:
    runner = CliRunner()
    r = runner.invoke(cli, ["info"])
    assert r.exit_code == 0
    assert "PyTorch" in r.output


def test_doctor() -> None:
    runner = CliRunner()
    r = runner.invoke(cli, ["doctor"])
    assert r.exit_code == 0
    assert "PyTorch" in r.output or "Diagnóstico" in r.output


def test_models() -> None:
    runner = CliRunner()
    r = runner.invoke(cli, ["models"])
    assert r.exit_code == 0
    assert "Hunyuan" in r.output or "Text2D" in r.output
