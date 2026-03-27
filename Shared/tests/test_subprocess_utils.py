"""Testes para gamedev_shared.subprocess_utils."""

import sys
from unittest.mock import patch

import pytest

from gamedev_shared.subprocess_utils import (
    RunResult,
    merge_subprocess_output,
    resolve_binary,
    run_cmd,
)


class TestRunResult:
    def test_ok_true(self):
        r = RunResult(returncode=0, stdout="ok", stderr="")
        assert r.ok is True

    def test_ok_false(self):
        r = RunResult(returncode=1, stdout="", stderr="erro")
        assert r.ok is False


class TestMergeSubprocessOutput:
    def test_only_stderr(self):
        r = RunResult(returncode=1, stdout="", stderr="erro")
        assert merge_subprocess_output(r) == "erro"

    def test_only_stdout(self):
        r = RunResult(returncode=0, stdout="saída", stderr="")
        assert merge_subprocess_output(r) == "saída"

    def test_both(self):
        r = RunResult(returncode=1, stdout="saída", stderr="erro")
        text = merge_subprocess_output(r)
        assert "erro" in text
        assert "saída" in text

    def test_truncate(self):
        r = RunResult(returncode=1, stdout="a" * 200, stderr="")
        text = merge_subprocess_output(r, max_chars=50)
        assert len(text) <= 200
        assert "truncado" in text

    def test_empty(self):
        r = RunResult(returncode=0, stdout="", stderr="")
        assert merge_subprocess_output(r) == ""


class TestResolveBinary:
    def test_env_override(self):
        with patch.dict("os.environ", {"MY_BIN": "/usr/bin/python3"}):
            assert resolve_binary("MY_BIN", "naoexiste") == "/usr/bin/python3"

    def test_which_fallback(self):
        result = resolve_binary("NAOEXISTE_ENV_VAR", "python3" if sys.platform != "win32" else "python")
        assert result

    def test_not_found(self):
        with (
            patch.dict("os.environ", {}, clear=False),
            pytest.raises(FileNotFoundError, match="Comando não encontrado"),
        ):
            resolve_binary("NADA_ENV", "comando_inexistente_xyz_123")


class TestRunCmd:
    def test_echo(self):
        if sys.platform == "win32":
            r = run_cmd(["python", "-c", "print('hello')"])
        else:
            r = run_cmd(["python3", "-c", "print('hello')"])
        assert r.ok
        assert "hello" in r.stdout

    def test_failure(self):
        if sys.platform == "win32":
            r = run_cmd(["python", "-c", "raise SystemExit(42)"])
        else:
            r = run_cmd(["python3", "-c", "raise SystemExit(42)"])
        assert not r.ok
        assert r.returncode == 42

    def test_extra_env(self):
        if sys.platform == "win32":
            r = run_cmd(
                ["python", "-c", "import os; print(os.environ['TEST_XYZ'])"],
                extra_env={"TEST_XYZ": "valor123"},
            )
        else:
            r = run_cmd(
                ["python3", "-c", "import os; print(os.environ['TEST_XYZ'])"],
                extra_env={"TEST_XYZ": "valor123"},
            )
        assert r.ok
        assert "valor123" in r.stdout
