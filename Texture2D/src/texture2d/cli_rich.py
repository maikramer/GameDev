"""Configuração Rich + rich-click para o CLI Texture2D (delegate para gamedev_shared)."""

from __future__ import annotations

from gamedev_shared.cli_rich import setup_rich_click

RICH_CLICK = setup_rich_click(
    header="[bold cyan]Texture2D[/bold cyan] — texturas 2D seamless · HF Inference API",
    footer="[dim]Documentação: README · Token: HF_TOKEN ou HUGGINGFACEHUB_API_TOKEN[/dim]",
)
