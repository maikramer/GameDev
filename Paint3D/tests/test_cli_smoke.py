"""Testes smoke para o CLI Paint3D (não requer GPU)."""

import subprocess
import sys


def test_paint3d_help():
    r = subprocess.run(
        [sys.executable, "-m", "paint3d", "--help"],
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert r.returncode == 0
    assert "paint3d" in r.stdout.lower() or "texture" in r.stdout.lower()


def test_paint3d_version():
    r = subprocess.run(
        [sys.executable, "-m", "paint3d", "--version"],
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert r.returncode == 0
    assert "0.1.0" in r.stdout
