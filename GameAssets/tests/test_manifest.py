"""Testes do manifest CSV."""

import tempfile
from pathlib import Path

import pytest

from gameassets.manifest import load_manifest


def test_load_basic() -> None:
    content = "id,idea,kind,generate_3d\na,idea one,prop,true\nb,idea two,,false\n"
    with tempfile.NamedTemporaryFile(
        mode="w", suffix=".csv", delete=False, encoding="utf-8"
    ) as f:
        f.write(content)
        path = Path(f.name)
    try:
        rows = load_manifest(path)
        assert len(rows) == 2
        assert rows[0].id == "a"
        assert rows[0].generate_3d is True
        assert rows[1].generate_3d is False
    finally:
        path.unlink(missing_ok=True)


def test_quoted_commas() -> None:
    content = (
        'id,idea,kind,generate_3d\n'
        'x,"hello, world",prop,sim\n'
    )
    with tempfile.NamedTemporaryFile(
        mode="w", suffix=".csv", delete=False, encoding="utf-8"
    ) as f:
        f.write(content)
        path = Path(f.name)
    try:
        rows = load_manifest(path)
        assert rows[0].idea == "hello, world"
    finally:
        path.unlink(missing_ok=True)


def test_missing_columns_raises() -> None:
    content = "a,b\n1,2\n"
    with tempfile.NamedTemporaryFile(
        mode="w", suffix=".csv", delete=False, encoding="utf-8"
    ) as f:
        f.write(content)
        path = Path(f.name)
    try:
        with pytest.raises(ValueError, match="id"):
            load_manifest(path)
    finally:
        path.unlink(missing_ok=True)
