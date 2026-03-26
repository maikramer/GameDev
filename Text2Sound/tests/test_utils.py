"""Testes para text2sound.utils."""

from text2sound.utils import resolve_effective_seed


def test_resolve_effective_seed_explicit():
    assert resolve_effective_seed(42) == 42
    assert resolve_effective_seed(0) == 0


def test_resolve_effective_seed_random():
    a = resolve_effective_seed(None)
    b = resolve_effective_seed(None)
    assert isinstance(a, int)
    assert isinstance(b, int)
    assert 0 <= a < 2**32
    assert 0 <= b < 2**32
