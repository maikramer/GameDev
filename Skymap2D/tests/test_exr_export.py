"""Testes para skymap2d.exr_export."""

import tempfile
from pathlib import Path

import numpy as np
from PIL import Image

from skymap2d.exr_export import pil_rgb_to_linear_f32, write_exr_rgb_linear


class TestPilRgbToLinear:
    def test_black_stays_zero(self) -> None:
        img = Image.new("RGB", (4, 4), (0, 0, 0))
        lin = pil_rgb_to_linear_f32(img)
        assert lin.shape == (4, 4, 3)
        np.testing.assert_allclose(lin, 0.0, atol=1e-6)

    def test_white_is_one(self) -> None:
        img = Image.new("RGB", (2, 2), (255, 255, 255))
        lin = pil_rgb_to_linear_f32(img)
        np.testing.assert_allclose(lin, 1.0, atol=1e-3)


class TestWriteExrRoundtrip:
    def test_write_and_read_rgb(self) -> None:
        import OpenEXR

        h, w = 8, 16
        rgb = np.random.rand(h, w, 3).astype(np.float32) * 0.5
        with tempfile.TemporaryDirectory() as td:
            p = Path(td) / "t.exr"
            write_exr_rgb_linear(p, rgb, scale=1.0)
            assert p.is_file()
            with OpenEXR.File(str(p)) as f:
                got = f.channels()["RGB"].pixels
            assert got.shape == (h, w, 3)
            np.testing.assert_allclose(got, rgb, rtol=1e-4, atol=1e-4)
