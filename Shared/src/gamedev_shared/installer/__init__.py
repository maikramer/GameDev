"""Instaladores base para projetos Python e Rust do monorepo GameDev."""

from .base import BaseInstaller, default_python_command
from .python_installer import PythonProjectInstaller
from .registry import (
    TOOLS,
    ToolKind,
    ToolSpec,
    find_monorepo_root,
    get_tool,
    list_available_tools,
    try_find_monorepo_root,
)
from .rust_installer import RustProjectInstaller

__all__ = [
    "TOOLS",
    "BaseInstaller",
    "PythonProjectInstaller",
    "RustProjectInstaller",
    "ToolKind",
    "ToolSpec",
    "default_python_command",
    "find_monorepo_root",
    "get_tool",
    "install_all",
    "install_tool",
    "list_available_tools",
    "try_find_monorepo_root",
]


def __getattr__(name: str) -> object:
    if name in ("install_tool", "install_all"):
        from .unified import install_all, install_tool

        return install_tool if name == "install_tool" else install_all
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
