"""Testes para gamedev_shared.installer.registry."""

import pytest
from pathlib import Path

from gamedev_shared.installer.registry import (
    ToolKind,
    ToolSpec,
    TOOLS,
    find_monorepo_root,
    list_available_tools,
    get_tool,
)


class TestToolSpec:
    def test_all_tools_registered(self):
        assert "text2d" in TOOLS
        assert "text3d" in TOOLS
        assert "gameassets" in TOOLS
        assert "materialize" in TOOLS

    def test_materialize_is_rust(self):
        spec = TOOLS["materialize"]
        assert spec.kind == ToolKind.RUST
        assert spec.cargo_bin_name == "materialize-cli"
        assert spec.cli_name == "materialize"

    def test_text2d_is_python(self):
        spec = TOOLS["text2d"]
        assert spec.kind == ToolKind.PYTHON
        assert spec.python_module == "text2d"
        assert spec.needs_pytorch is True

    def test_gameassets_no_pytorch(self):
        spec = TOOLS["gameassets"]
        assert spec.needs_pytorch is False

    def test_project_root(self, tmp_path: Path):
        spec = ToolSpec(
            name="Test",
            kind=ToolKind.PYTHON,
            folder="MyTool",
            cli_name="mytool",
            description="test",
        )
        assert spec.project_root(tmp_path) == tmp_path / "MyTool"

    def test_exists_python(self, tmp_path: Path):
        spec = ToolSpec(
            name="Test",
            kind=ToolKind.PYTHON,
            folder="PyTool",
            cli_name="pytool",
            description="test",
        )
        assert spec.exists(tmp_path) is False

        (tmp_path / "PyTool").mkdir()
        (tmp_path / "PyTool" / "setup.py").touch()
        assert spec.exists(tmp_path) is True

    def test_exists_rust(self, tmp_path: Path):
        spec = ToolSpec(
            name="Test",
            kind=ToolKind.RUST,
            folder="RustTool",
            cli_name="rtool",
            description="test",
            cargo_bin_name="rtool-cli",
        )
        assert spec.exists(tmp_path) is False

        (tmp_path / "RustTool").mkdir()
        (tmp_path / "RustTool" / "Cargo.toml").touch()
        assert spec.exists(tmp_path) is True


class TestFindMonorepoRoot:
    def test_finds_root(self):
        root = find_monorepo_root()
        assert (root / ".git").exists()
        assert (root / "Shared").is_dir()

    def test_raises_if_not_found(self, tmp_path: Path):
        with pytest.raises(FileNotFoundError, match="Raiz do monorepo"):
            find_monorepo_root(tmp_path / "nonexistent")


class TestListAvailableTools:
    def test_returns_list(self):
        tools = list_available_tools()
        assert len(tools) >= 1
        names = {t.cli_name for t in tools}
        assert "materialize" in names

    def test_empty_monorepo(self, tmp_path: Path):
        tools = list_available_tools(tmp_path)
        assert tools == []


class TestGetTool:
    def test_by_cli_name(self):
        spec = get_tool("materialize")
        assert spec.name == "Materialize CLI"

    def test_by_key(self):
        spec = get_tool("text2d")
        assert spec.cli_name == "text2d"

    def test_case_insensitive(self):
        spec = get_tool("Materialize")
        assert spec.kind == ToolKind.RUST

    def test_unknown_raises(self):
        with pytest.raises(KeyError, match="Ferramenta desconhecida"):
            get_tool("nonexistent_tool")

    def test_gameassets_variants(self):
        spec1 = get_tool("gameassets")
        spec2 = get_tool("GameAssets")
        assert spec1.name == spec2.name
