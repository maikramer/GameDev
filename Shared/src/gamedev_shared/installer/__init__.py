"""Instaladores base para projetos Python e Rust do monorepo GameDev."""

from .base import BaseInstaller
from .python_installer import PythonProjectInstaller
from .rust_installer import RustProjectInstaller
from .registry import ToolKind, ToolSpec, TOOLS, find_monorepo_root, list_available_tools, get_tool

__all__ = [
    "BaseInstaller",
    "PythonProjectInstaller",
    "RustProjectInstaller",
    "ToolKind",
    "ToolSpec",
    "TOOLS",
    "find_monorepo_root",
    "list_available_tools",
    "get_tool",
    "install_tool",
    "install_all",
]


def __getattr__(name: str):
    if name in ("install_tool", "install_all"):
        from .unified import install_tool, install_all  # noqa: F811
        return install_tool if name == "install_tool" else install_all
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
