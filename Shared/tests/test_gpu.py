"""Testes para gamedev_shared.gpu (funções sem dependência de GPU real)."""

import pytest

from gamedev_shared.gpu import format_bytes, estimate_vram_requirement


class TestFormatBytes:
    def test_bytes(self):
        assert format_bytes(500) == "500.0 B"

    def test_kilobytes(self):
        assert format_bytes(1024) == "1.0 KB"

    def test_megabytes(self):
        assert format_bytes(1024 * 1024) == "1.0 MB"

    def test_gigabytes(self):
        assert format_bytes(1024**3) == "1.0 GB"

    def test_terabytes(self):
        assert format_bytes(1024**4) == "1.0 TB"

    def test_fractional(self):
        result = format_bytes(int(4.5 * 1024**3))
        assert "GB" in result

    def test_zero(self):
        assert format_bytes(0) == "0.0 B"


class TestEstimateVram:
    def test_default(self):
        est = estimate_vram_requirement()
        assert est > 0
        assert est == pytest.approx(4.9 * 1.2, rel=0.01)

    def test_larger_frame(self):
        base = estimate_vram_requirement(frame_size=256)
        larger = estimate_vram_requirement(frame_size=512)
        assert larger > base

    def test_batch_scales(self):
        single = estimate_vram_requirement(batch_size=1)
        double = estimate_vram_requirement(batch_size=2)
        assert double == pytest.approx(single * 2, rel=0.01)
