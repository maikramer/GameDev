"""Configuração partilhada Rich + rich-click para o CLI Texture2D."""

from __future__ import annotations

try:
    import rich_click.rich_click as _rc

    _rc.USE_RICH_MARKUP = True
    _rc.GROUP_ARGUMENTS_OPTIONS = True
    _rc.SHOW_METAVARS_COLUMN = True
    _rc.HEADER_TEXT = (
        "[bold cyan]Texture2D[/bold cyan] — texturas 2D seamless · HF Inference API"
    )
    _rc.FOOTER_TEXT = (
        "[dim]Documentação: README · Token: HF_TOKEN ou HUGGINGFACEHUB_API_TOKEN[/dim]"
    )
    RICH_CLICK = True
except ImportError:
    RICH_CLICK = False
