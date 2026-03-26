"""Testes para text2sound.presets."""

import pytest

from text2sound.presets import AUDIO_PRESETS, get_preset, list_presets


class TestListPresets:
    def test_returns_sorted(self):
        names = list_presets()
        assert names == sorted(names)

    def test_not_empty(self):
        assert len(list_presets()) > 0

    def test_known_presets(self):
        names = list_presets()
        assert "ambient" in names
        assert "battle" in names
        assert "menu" in names
        assert "rain" in names
        assert "explosion" in names


class TestGetPreset:
    def test_exact_match(self):
        p = get_preset("ambient")
        assert "prompt" in p
        assert "duration" in p
        assert "steps" in p
        assert "cfg_scale" in p

    def test_case_insensitive(self):
        p = get_preset("BATTLE")
        assert "prompt" in p

    def test_underscore_to_hyphen(self):
        p = get_preset("footsteps_stone")
        assert "prompt" in p

    def test_unknown_raises(self):
        with pytest.raises(KeyError, match="Preset desconhecido"):
            get_preset("nao_existe_este_preset")


class TestPresetStructure:
    @pytest.mark.parametrize("name", list_presets())
    def test_required_fields(self, name):
        p = AUDIO_PRESETS[name]
        assert isinstance(p["prompt"], str) and len(p["prompt"]) > 0
        assert isinstance(p["duration"], (int, float)) and p["duration"] > 0
        assert isinstance(p["steps"], int) and p["steps"] > 0
        assert isinstance(p["cfg_scale"], (int, float)) and p["cfg_scale"] > 0

    @pytest.mark.parametrize("name", list_presets())
    def test_duration_within_model_limits(self, name):
        p = AUDIO_PRESETS[name]
        assert 0 < p["duration"] <= 47

    @pytest.mark.parametrize("name", list_presets())
    def test_steps_reasonable(self, name):
        p = AUDIO_PRESETS[name]
        assert 10 <= p["steps"] <= 200

    @pytest.mark.parametrize("name", list_presets())
    def test_cfg_scale_reasonable(self, name):
        p = AUDIO_PRESETS[name]
        assert 1.0 <= p["cfg_scale"] <= 15.0
