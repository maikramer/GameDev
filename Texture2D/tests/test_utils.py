"""Testes para texture2d.utils."""

import pytest

from texture2d.utils import (
    format_bytes,
    generate_seed,
    validate_dimensions,
    validate_params,
    validate_prompt,
)


class TestGenerateSeed:
    def test_returns_int(self):
        seed = generate_seed()
        assert isinstance(seed, int)

    def test_within_range(self):
        for _ in range(50):
            seed = generate_seed()
            assert 0 <= seed < 2**32


class TestValidatePrompt:
    def test_valid_prompt(self):
        ok, err = validate_prompt("stone wall texture")
        assert ok is True
        assert err is None

    def test_empty_prompt(self):
        ok, err = validate_prompt("")
        assert ok is False
        assert err is not None

    def test_whitespace_prompt(self):
        ok, err = validate_prompt("   ")
        assert ok is False

    def test_exceeds_max_length(self):
        ok, err = validate_prompt("a" * 600, max_length=500)
        assert ok is False
        assert "500" in str(err)


class TestValidateDimensions:
    def test_valid(self):
        ok, err = validate_dimensions(1024, 1024)
        assert ok is True

    def test_too_small(self):
        ok, err = validate_dimensions(128, 1024)
        assert ok is False

    def test_too_large(self):
        ok, err = validate_dimensions(1024, 4096)
        assert ok is False

    def test_not_multiple_of_8(self):
        ok, err = validate_dimensions(1023, 1024)
        assert ok is False


class TestValidateParams:
    def test_valid_defaults(self):
        ok, err = validate_params({"guidance_scale": 7.5, "num_inference_steps": 50})
        assert ok is True

    def test_guidance_too_high(self):
        ok, err = validate_params({"guidance_scale": 25.0})
        assert ok is False

    def test_steps_too_low(self):
        ok, err = validate_params({"num_inference_steps": 5})
        assert ok is False


class TestFormatBytes:
    def test_bytes(self):
        assert "500.0 B" == format_bytes(500)

    def test_kilobytes(self):
        result = format_bytes(2048)
        assert "KB" in result

    def test_megabytes(self):
        result = format_bytes(5 * 1024 * 1024)
        assert "MB" in result
