"""Configuração partilhada Rich + rich-click para o CLI Text3D."""

from __future__ import annotations

try:
    import rich_click.rich_click as _rc

    _rc.USE_RICH_MARKUP = True
    _rc.GROUP_ARGUMENTS_OPTIONS = True
    _rc.SHOW_METAVARS_COLUMN = True
    _rc.HEADER_TEXT = "[bold cyan]Text3D[/bold cyan] — Text2D + Hunyuan3D · mesh a partir de texto"
    _rc.FOOTER_TEXT = "[dim]Primeira execução: downloads HF · text3d doctor · docs/[/dim]"
    RICH_CLICK = True
except ImportError:
    RICH_CLICK = False
