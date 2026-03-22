"""Configuração partilhada Rich + rich-click para o CLI GameAssets."""

from __future__ import annotations

try:
    import rich_click.rich_click as _rc

    _rc.USE_RICH_MARKUP = True
    _rc.GROUP_ARGUMENTS_OPTIONS = True
    _rc.SHOW_METAVARS_COLUMN = True
    _rc.HEADER_TEXT = (
        "[bold cyan]GameAssets[/bold cyan] — batch de prompts e assets alinhados ao jogo"
    )
    _rc.FOOTER_TEXT = (
        "[dim]TEXT2D_BIN / TEXT3D_BIN / MATERIALIZE_BIN se os comandos não estiverem no PATH[/dim]"
    )
    RICH_CLICK = True
except ImportError:
    RICH_CLICK = False
