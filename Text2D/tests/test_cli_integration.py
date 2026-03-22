"""Comandos CLI adicionais (versão, generate --help)."""

from __future__ import annotations

from click.testing import CliRunner

from text2d.cli import cli


def test_version() -> None:
    runner = CliRunner()
    r = runner.invoke(cli, ["--version"])
    assert r.exit_code == 0
    assert "0.1.0" in r.output


def test_verbose_flag_on_group() -> None:
    runner = CliRunner()
    r = runner.invoke(cli, ["-v", "--help"])
    assert r.exit_code == 0


def test_generate_help_lists_options() -> None:
    runner = CliRunner()
    r = runner.invoke(cli, ["generate", "--help"])
    assert r.exit_code == 0
    assert "--seed" in r.output or "seed" in r.output.lower()
