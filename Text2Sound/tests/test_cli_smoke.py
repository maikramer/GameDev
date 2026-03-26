"""Smoke tests para text2sound CLI (sem modelo/GPU)."""

import pytest

pytest.importorskip("torch")
pytest.importorskip("torchaudio")
pytest.importorskip("stable_audio_tools")

from click.testing import CliRunner

from text2sound.cli import cli


@pytest.fixture
def runner():
    return CliRunner()


class TestCLISmoke:
    def test_help(self, runner):
        result = runner.invoke(cli, ["--help"])
        assert result.exit_code == 0
        assert "text2sound" in result.output.lower() or "Text2Sound" in result.output

    def test_version(self, runner):
        result = runner.invoke(cli, ["--version"])
        assert result.exit_code == 0
        assert "0.1.0" in result.output

    def test_generate_help(self, runner):
        result = runner.invoke(cli, ["generate", "--help"])
        assert result.exit_code == 0
        assert "prompt" in result.output.lower()
        assert "--duration" in result.output
        assert "--steps" in result.output
        assert "--cfg-scale" in result.output

    def test_batch_help(self, runner):
        result = runner.invoke(cli, ["batch", "--help"])
        assert result.exit_code == 0
        assert "file" in result.output.lower()

    def test_presets_command(self, runner):
        result = runner.invoke(cli, ["presets"])
        assert result.exit_code == 0
        assert "ambient" in result.output
        assert "battle" in result.output

    def test_info_command(self, runner):
        result = runner.invoke(cli, ["info"])
        assert result.exit_code == 0
        assert "stable-audio-open-1.0" in result.output or "44100" in result.output

    def test_skill_help(self, runner):
        result = runner.invoke(cli, ["skill", "--help"])
        assert result.exit_code == 0
        assert "install" in result.output


class TestGenerateValidation:
    def test_duration_too_high(self, runner):
        result = runner.invoke(cli, ["generate", "test", "--duration", "100"])
        assert result.exit_code != 0

    def test_duration_too_low(self, runner):
        result = runner.invoke(cli, ["generate", "test", "--duration", "0"])
        assert result.exit_code != 0

    def test_steps_too_low(self, runner):
        result = runner.invoke(cli, ["generate", "test", "--steps", "1"])
        assert result.exit_code != 0

    def test_steps_too_high(self, runner):
        result = runner.invoke(cli, ["generate", "test", "--steps", "999"])
        assert result.exit_code != 0

    def test_invalid_format(self, runner):
        result = runner.invoke(cli, ["generate", "test", "--format", "mp3"])
        assert result.exit_code != 0
