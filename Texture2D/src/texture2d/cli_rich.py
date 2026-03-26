"""Configuração Rich + rich-click para o CLI Texture2D."""

from __future__ import annotations

from typing import Final

_HEADER: Final = (
    "[bold cyan]Texture2D[/bold cyan] — texturas 2D seamless · HF Inference API"
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

        _rc.USE_RICH_MARKUP = use_rich_markup
        _rc.GROUP_ARGUMENTS_OPTIONS = group_arguments_options
        _rc.SHOW_METAVARS_COLUMN = show_metavars_column
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
