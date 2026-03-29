"""rich-click como `click` — mesmo padrão que Rigging3D."""

from __future__ import annotations

import rich_click as click

click.rich_click.USE_RICH_MARKUP = True
click.rich_click.USE_MARKDOWN = True
click.rich_click.SHOW_ARGUMENTS = True

__all__ = ["click"]
