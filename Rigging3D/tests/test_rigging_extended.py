"""Testes extra Rigging3D: CLI, env, validação IO."""

from __future__ import annotations

from pathlib import Path
from unittest.mock import patch

import pytest
from click import ClickException
from click.testing import CliRunner
from rigging3d import __version__
from rigging3d.cli import (
    _find_bash,
    _io_args,
    _make_env,
    _resolve_python,
    _resolve_root,
    _shell_path,
    _validate_io,
    cli,
)


def _tree(p: Path) -> None:
    (p / "configs").mkdir(parents=True)
    (p / "src").mkdir()


def test_version_string() -> None:
    assert len(__version__) >= 1


def test_cli_skeleton_help() -> None:
    r = CliRunner().invoke(cli, ["skeleton", "--help"])
    assert r.exit_code == 0
    assert "--input" in r.output or "-i" in r.output


def test_cli_skin_help() -> None:
    r = CliRunner().invoke(cli, ["skin", "--help"])
    assert r.exit_code == 0


def test_cli_merge_help() -> None:
    r = CliRunner().invoke(cli, ["merge", "--help"])
    assert r.exit_code == 0
    assert "--source" in r.output


def test_cli_pipeline_help() -> None:
    r = CliRunner().invoke(cli, ["pipeline", "--help"])
    assert r.exit_code == 0


def test_cli_root_help() -> None:
    r = CliRunner().invoke(cli, ["--help"])
    assert r.exit_code == 0
    assert "rigging3d" in r.output.lower()


def test_validate_io_dir_without_output_dir() -> None:
    with pytest.raises(ClickException, match="output-dir"):
        _validate_io(None, None, Path("/in"), None)


def test_validate_io_no_args() -> None:
    with pytest.raises(ClickException, match="--input"):
        _validate_io(None, None, None, None)


def test_validate_io_input_only(tmp_path: Path) -> None:
    with pytest.raises(ClickException):
        _validate_io(tmp_path / "i.glb", None, None, None)


def test_io_args_single_file(tmp_path: Path) -> None:
    inp = tmp_path / "i.glb"
    out = tmp_path / "o.glb"
    inp.write_bytes(b"x")
    args = _io_args(inp, out, None, None)
    assert "--input" in args and "--output" in args


def test_io_args_dir_mode(tmp_path: Path) -> None:
    idir = tmp_path / "in"
    odir = tmp_path / "out"
    idir.mkdir()
    odir.mkdir()
    args = _io_args(None, None, idir, odir)
    assert "--input_dir" in args


def test_resolve_python_explicit() -> None:
    assert _resolve_python("/usr/bin/python3") == "/usr/bin/python3"


def test_resolve_python_env(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("RIGGING3D_PYTHON", "/venv/bin/python")
    assert _resolve_python(None) == "/venv/bin/python"


def test_make_env_prepends_pythonpath(tmp_path: Path) -> None:
    root = tmp_path / "rr"
    env = _make_env(root, extra={"FOO": "bar"})
    assert "FOO" in env and env["FOO"] == "bar"
    assert str(root) in env.get("PYTHONPATH", "")


def test_shell_path_posix() -> None:
    p = _shell_path(Path("/tmp/x.sh"))
    assert isinstance(p, str) and len(p) > 0


def test_find_bash_type() -> None:
    b = _find_bash()
    assert b is None or isinstance(b, str)


def test_resolve_root_explicit(tmp_path: Path) -> None:
    root = tmp_path / "r"
    _tree(root)
    assert _resolve_root(root) == root.resolve()


def test_cli_root_version() -> None:
    r = CliRunner().invoke(cli, ["--version"])
    assert r.exit_code == 0
    assert __version__ in r.output


def test_merge_cmd_requires_args() -> None:
    r = CliRunner().invoke(cli, ["merge"], catch_exceptions=False)
    assert r.exit_code != 0


def test_skin_requires_bash(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    ur = tmp_path / "u"
    _tree(ur)
    monkeypatch.setenv("RIGGING3D_ROOT", str(ur))
    with patch("rigging3d.cli._find_bash", return_value=None):
        r = CliRunner().invoke(
            cli,
            ["skin", "--data-name", "d", "-i", "a.glb", "-o", "b.glb"],
            catch_exceptions=False,
        )
    assert r.exit_code != 0


def test_skeleton_requires_bash(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    ur = tmp_path / "u"
    _tree(ur)
    monkeypatch.setenv("RIGGING3D_ROOT", str(ur))
    with patch("rigging3d.cli._find_bash", return_value=None):
        r = CliRunner().invoke(
            cli,
            ["skeleton", "-i", "mesh.glb", "-o", "skel.glb"],
            catch_exceptions=False,
        )
    assert r.exit_code != 0
