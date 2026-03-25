"""Configuração parametrizada de rich-click para CLIs do monorepo GameDev."""

from __future__ import annotations


def setup_rich_click(
    header: str,
    footer: str,
    *,
    use_rich_markup: bool = True,
    group_arguments_options: bool = True,
    show_metavars_column: bool = True,
) -> bool:
    """Configura rich-click com header/footer personalizados.

    Retorna ``True`` se rich-click estiver disponível e configurado,
    ``False`` caso contrário (graceful degradation para Click puro).

    Deve ser chamado **antes** de importar Click nos módulos CLI::

        from gamedev_shared.cli_rich import setup_rich_click

        RICH_CLICK = setup_rich_click(
            header="[bold cyan]Text2D[/bold cyan] — FLUX.2 Klein",
            footer="[dim]Docs: README[/dim]",
        )

        if RICH_CLICK:
            import rich_click as click
        else:
            import click
    """
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
