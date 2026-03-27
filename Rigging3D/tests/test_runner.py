"""Testes de resolução de caminhos, env e utilitários internos."""

from __future__ import annotations

import os
import sys
from pathlib import Path

import pytest
from rigging3d.cli import (
    _find_bash,
    _io_args,
    _make_env,
    _package_root,
    _resolve_python,
    _resolve_root,
    _shell_path,
    _validate_io,
)


def _tree(p: Path) -> None:
    (p / "configs").mkdir(parents=True)
    (p / "src").mkdir()


# ── _package_root ──────────────────────────────────────────────────────


class TestPackageRoot:
    def test_points_to_unirig(self) -> None:
        assert _package_root().name == "unirig"

    def test_has_configs_and_src(self) -> None:
        root = _package_root()
        assert (root / "configs").is_dir()
        assert (root / "src").is_dir()

    def test_has_run_py(self) -> None:
        assert (_package_root() / "run.py").is_file()

    def test_has_launch_scripts(self) -> None:
        launch = _package_root() / "launch" / "inference"
        assert (launch / "generate_skeleton.sh").is_file()
        assert (launch / "generate_skin.sh").is_file()
        assert (launch / "extract.sh").is_file()

    def test_has_license(self) -> None:
        assert (_package_root() / "LICENSE").is_file()


# ── _resolve_root ──────────────────────────────────────────────────────


class TestResolveRoot:
    def test_explicit_path(self, tmp_path: Path) -> None:
        root = tmp_path / "r"
        _tree(root)
        assert _resolve_root(root) == root.resolve()

    def test_env_var(self, tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
        root = tmp_path / "r"
        _tree(root)
        monkeypatch.setenv("RIGGING3D_ROOT", str(root))
        assert _resolve_root(None) == root.resolve()

    def test_default_package(self, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.delenv("RIGGING3D_ROOT", raising=False)
        assert _resolve_root(None) == _package_root()

    def test_explicit_takes_precedence_over_env(
        self, tmp_path: Path, monkeypatch: pytest.MonkeyPatch,
    ) -> None:
        env_root = tmp_path / "env"
        _tree(env_root)
        explicit = tmp_path / "explicit"
        _tree(explicit)
        monkeypatch.setenv("RIGGING3D_ROOT", str(env_root))
        assert _resolve_root(explicit) == explicit.resolve()

    def test_missing_configs_raises(self, tmp_path: Path) -> None:
        bad = tmp_path / "bad"
        (bad / "src").mkdir(parents=True)
        with pytest.raises(FileNotFoundError, match="configs"):
            _resolve_root(bad)

    def test_missing_src_raises(self, tmp_path: Path) -> None:
        bad = tmp_path / "bad"
        (bad / "configs").mkdir(parents=True)
        with pytest.raises(FileNotFoundError, match="src"):
            _resolve_root(bad)

    def test_empty_dir_raises(self, tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.delenv("RIGGING3D_ROOT", raising=False)
        empty = tmp_path / "empty"
        empty.mkdir()
        with pytest.raises(FileNotFoundError):
            _resolve_root(empty)

    def test_env_whitespace_stripped(self, tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
        root = tmp_path / "r"
        _tree(root)
        monkeypatch.setenv("RIGGING3D_ROOT", f"  {root}  ")
        assert _resolve_root(None) == root.resolve()

    def test_tilde_expanded(self, tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
        root = tmp_path / "r"
        _tree(root)
        monkeypatch.setenv("HOME", str(tmp_path))
        monkeypatch.setenv("USERPROFILE", str(tmp_path))
        assert _resolve_root(Path("~/r")) == root.resolve()


# ── _resolve_python ────────────────────────────────────────────────────


class TestResolvePython:
    def test_explicit_wins(self) -> None:
        assert _resolve_python("/usr/bin/python3") == "/usr/bin/python3"

    def test_env_var(self, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.setenv("RIGGING3D_PYTHON", "/custom/python")
        assert _resolve_python(None) == "/custom/python"

    def test_default_sys_executable(self, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.delenv("RIGGING3D_PYTHON", raising=False)
        assert _resolve_python(None) == sys.executable

    def test_explicit_over_env(self, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.setenv("RIGGING3D_PYTHON", "/env/python")
        assert _resolve_python("/explicit/python") == "/explicit/python"

    def test_env_whitespace_stripped(self, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.setenv("RIGGING3D_PYTHON", "  /custom/python  ")
        assert _resolve_python(None) == "/custom/python"

    def test_empty_env_falls_back(self, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.setenv("RIGGING3D_PYTHON", "")
        assert _resolve_python(None) == sys.executable


# ── _shell_path ────────────────────────────────────────────────────────


class TestShellPath:
    def test_no_backslash_on_win32(self, tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.setattr(sys, "platform", "win32")
        f = tmp_path / "mesh.glb"
        f.write_bytes(b"x")
        assert "\\" not in _shell_path(f)

    def test_resolves_to_absolute(self, tmp_path: Path) -> None:
        f = tmp_path / "mesh.glb"
        f.write_bytes(b"x")
        result = _shell_path(f)
        assert os.path.isabs(result)


# ── _make_env ──────────────────────────────────────────────────────────


class TestMakeEnv:
    def test_sets_pythonpath(self, tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.delenv("PYTHONPATH", raising=False)
        e = _make_env(tmp_path)
        assert e["PYTHONPATH"] == str(tmp_path)

    def test_prepends_to_existing_pythonpath(self, tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.setenv("PYTHONPATH", "/existing")
        e = _make_env(tmp_path)
        parts = e["PYTHONPATH"].split(os.pathsep)
        assert parts[0] == str(tmp_path)
        assert "/existing" in parts

    def test_extra_dict_merged(self, tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.delenv("PYTHONPATH", raising=False)
        e = _make_env(tmp_path, extra={"MY_VAR": "42"})
        assert e["MY_VAR"] == "42"
        assert e["PYTHONPATH"] == str(tmp_path)

    def test_preserves_existing_env(self, tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.setenv("SOME_KEY", "value")
        monkeypatch.delenv("PYTHONPATH", raising=False)
        e = _make_env(tmp_path)
        assert e["SOME_KEY"] == "value"


# ── _find_bash ─────────────────────────────────────────────────────────


class TestFindBash:
    def test_returns_string_or_none(self) -> None:
        result = _find_bash()
        assert result is None or isinstance(result, str)

    def test_unix_finds_bash(self, monkeypatch: pytest.MonkeyPatch) -> None:
        import shutil

        import rigging3d.cli as cli_mod

        monkeypatch.setattr(cli_mod, "_WIN32", False)
        monkeypatch.setattr(shutil, "which", lambda _cmd: "/usr/bin/bash")
        assert _find_bash() == "/usr/bin/bash"

    def test_unix_no_bash(self, monkeypatch: pytest.MonkeyPatch) -> None:
        import shutil

        import rigging3d.cli as cli_mod

        monkeypatch.setattr(cli_mod, "_WIN32", False)
        monkeypatch.setattr(shutil, "which", lambda _cmd: None)
        assert _find_bash() is None


# ── _validate_io ───────────────────────────────────────────────────────


class TestValidateIO:
    def test_single_file_ok(self, tmp_path: Path) -> None:
        _validate_io(tmp_path / "i.glb", tmp_path / "o.fbx", None, None)

    def test_dir_mode_ok(self, tmp_path: Path) -> None:
        _validate_io(None, None, tmp_path / "in", tmp_path / "out")

    def test_input_dir_without_output_dir_fails(self, tmp_path: Path) -> None:
        from click import ClickException

        with pytest.raises(ClickException, match="output-dir"):
            _validate_io(None, None, tmp_path / "in", None)

    def test_no_args_fails(self) -> None:
        from click import ClickException

        with pytest.raises(ClickException, match="--input"):
            _validate_io(None, None, None, None)

    def test_input_only_fails(self, tmp_path: Path) -> None:
        from click import ClickException

        with pytest.raises(ClickException):
            _validate_io(tmp_path / "i.glb", None, None, None)

    def test_output_only_fails(self, tmp_path: Path) -> None:
        from click import ClickException

        with pytest.raises(ClickException):
            _validate_io(None, tmp_path / "o.fbx", None, None)


# ── _io_args ───────────────────────────────────────────────────────────


class TestIOArgs:
    def test_single_file_args(self, tmp_path: Path) -> None:
        inp = tmp_path / "i.glb"
        out = tmp_path / "o.fbx"
        inp.write_bytes(b"x")
        args = _io_args(inp, out, None, None)
        assert "--input" in args
        assert "--output" in args

    def test_dir_args(self, tmp_path: Path) -> None:
        idir = tmp_path / "in"
        odir = tmp_path / "out"
        idir.mkdir()
        odir.mkdir()
        args = _io_args(None, None, idir, odir)
        assert "--input_dir" in args
        assert "--output_dir" in args
        assert "--input" not in args

    def test_single_file_with_output_dir(self, tmp_path: Path) -> None:
        inp = tmp_path / "i.glb"
        out = tmp_path / "o.fbx"
        odir = tmp_path / "out"
        inp.write_bytes(b"x")
        odir.mkdir()
        args = _io_args(inp, out, None, odir)
        assert "--input" in args
        assert "--output" in args
        assert "--output_dir" in args
