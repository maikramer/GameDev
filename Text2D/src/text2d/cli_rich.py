"""Configuração partilhada Rich + rich-click para o CLI Text2D."""

from __future__ import annotations

try:
    # rich_click ≥2: config vive no módulo (não há símbolo `rich_click` exportado).
    import rich_click.rich_click as _rc

    _rc.USE_RICH_MARKUP = True
    _rc.GROUP_ARGUMENTS_OPTIONS = True
    _rc.SHOW_METAVARS_COLUMN = True
    _rc.HEADER_TEXT = "[bold cyan]Text2D[/bold cyan] — FLUX.2 Klein · texto → imagem"
    _rc.FOOTER_TEXT = (
        "[dim]Documentação: README / docs · Cache HF: ~/.cache/huggingface[/dim]"
    )
    RICH_CLICK = True
except ImportError:
    RICH_CLICK = False
