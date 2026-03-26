"""Configuração Rich + rich-click para o CLI Skymap2D."""

from __future__ import annotations

from typing import Final

_HEADER: Final = (
    "[bold cyan]Skymap2D[/bold cyan] — skymaps equirectangular 360° · HF Inference API"
)
_FOOTER: Final = (
    "[dim]Documentação: README · Token: HF_TOKEN ou HUGGINGFACEHUB_API_TOKEN[/dim]"
)


def _setup_rich_click_local(
    header: str,
    footer: str,
    *,
    use_rich_markup: bool = True,
    group_arguments_options: bool = True,
    show_metavars_column: bool = True,
) -> bool:
    """Mesma lógica que ``gamedev_shared.cli_rich.setup_rich_click`` (fallback)."""
    try:
        import rich_click.rich_click as _rc

        _rc.TEXT_MARKUP = "rich" if use_rich_markup else "ansi"
        _rc.GROUP_ARGUMENTS_OPTIONS = group_arguments_options
        if not show_metavars_column:
            _rc.OPTIONS_TABLE_COLUMN_TYPES = [
                "required",
                "opt_short",
                "opt_long",
                "help",
            ]
        _rc.HEADER_TEXT = header
        _rc.FOOTER_TEXT = footer
        return True
    except ImportError:
        return False


try:
    from gamedev_shared.cli_rich import setup_rich_click as _setup
except ImportError:
    _setup = _setup_rich_click_local

RICH_CLICK = _setup(header=_HEADER, footer=_FOOTER)
