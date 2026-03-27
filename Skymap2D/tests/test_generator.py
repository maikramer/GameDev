"""Testes para skymap2d.generator."""

from unittest.mock import MagicMock, patch

from PIL import Image

from skymap2d.generator import (
    DEFAULT_MODEL_ID,
    augment_prompt_for_equirectangular,
    default_model_id,
    merge_negative_prompt,
)


class TestAugmentPrompt:
    def test_adds_equirectangular_prefix(self):
        result = augment_prompt_for_equirectangular("sunset sky")
        assert "equirectangular" in result.lower()
        assert "sunset sky" in result

    def test_skips_if_already_equirectangular(self):
        original = "equirectangular sunset panorama"
        result = augment_prompt_for_equirectangular(original)
        assert result == original

    def test_skips_if_panorama(self):
        original = "panorama of a mountain range"
        result = augment_prompt_for_equirectangular(original)
        assert result == original

    def test_skips_if_360(self):
        original = "360 degree view of ocean"
        result = augment_prompt_for_equirectangular(original)
        assert result == original

    def test_skips_if_hdri(self):
        original = "hdri sky environment"
        result = augment_prompt_for_equirectangular(original)
        assert result == original

    def test_empty_prompt(self):
        assert augment_prompt_for_equirectangular("") == ""
        assert augment_prompt_for_equirectangular("   ") == ""


class TestMergeNegativePrompt:
    def test_only_preset(self):
        result = merge_negative_prompt("indoor", "")
        assert result == "indoor"

    def test_only_user(self):
        result = merge_negative_prompt("", "low quality")
        assert result == "low quality"

    def test_both_different(self):
        result = merge_negative_prompt("indoor", "low quality")
        assert "indoor" in result
        assert "low quality" in result

    def test_subset_dedup(self):
        result = merge_negative_prompt("indoor", "indoor, low quality")
        assert result == "indoor, low quality"


class TestDefaultModelId:
    def test_default(self):
        assert default_model_id() == DEFAULT_MODEL_ID

    @patch.dict("os.environ", {"SKYMAP2D_MODEL_ID": "custom/model"})
    def test_env_override(self):
        assert default_model_id() == "custom/model"


class TestSkymapGenerator:
    @patch("skymap2d.generator.SkymapGenerator._init_client")
    def test_init(self, mock_client):
        from skymap2d.generator import SkymapGenerator

        mock_client.return_value = MagicMock()
        gen = SkymapGenerator()
        assert gen.model_id == DEFAULT_MODEL_ID

    @patch("skymap2d.generator.SkymapGenerator._init_client")
    def test_generate_returns_image(self, mock_init):
        from skymap2d.generator import SkymapGenerator

        mock_image = Image.new("RGB", (128, 64), color="blue")
        mock_client = MagicMock()
        mock_client.text_to_image.return_value = mock_image
        mock_init.return_value = mock_client

        gen = SkymapGenerator()
        image, metadata = gen.generate(
            prompt="test sunset sky",
            width=1024,
            height=512,
            num_inference_steps=10,
        )
        assert isinstance(image, Image.Image)
        assert "seed" in metadata
        mock_client.text_to_image.assert_called_once()
