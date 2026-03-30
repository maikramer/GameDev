"""Smoke tests do CLI Part3D (sem GPU nem pesos)."""

from __future__ import annotations

from click.testing import CliRunner

from part3d.cli import main


def test_help_shows_group_and_decompose() -> None:
    runner = CliRunner()
    result = runner.invoke(main, ["--help"])
    assert result.exit_code == 0
    assert "Part3D" in result.output or "part3d" in result.output.lower()
    assert "decompose" in result.output


def test_version() -> None:
    runner = CliRunner()
    result = runner.invoke(main, ["--version"])
    assert result.exit_code == 0
    assert "0.1.0" in result.output
