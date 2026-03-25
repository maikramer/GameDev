"""Instalador unificado do monorepo GameDev.

Fornece ``install_tool`` e ``UnifiedInstaller`` — interface única para
instalar qualquer ferramenta (Python ou Rust) registada no registry.
"""

from __future__ import annotations

import argparse
import platform
import sys
from pathlib import Path
from typing import Optional

from ..logging import Logger
from .registry import ToolKind, ToolSpec, TOOLS, find_monorepo_root, list_available_tools, get_tool
from .python_installer import PythonProjectInstaller
from .rust_installer import RustProjectInstaller


class _ToolPythonInstaller(PythonProjectInstaller):
    """Adaptador: instancia PythonProjectInstaller a partir de um ToolSpec."""

    def __init__(
        self,
        spec: ToolSpec,
        monorepo: Path,
        *,
        install_prefix: Path | None = None,
        python_cmd: str = "python3",
        use_venv: bool = False,
        skip_deps: bool = False,
        skip_models: bool = False,
        force: bool = False,
    ) -> None:
        super().__init__(
            project_name=spec.name,
            cli_name=spec.cli_name,
            project_root=spec.project_root(monorepo),
            install_prefix=install_prefix,
            python_cmd=python_cmd,
            use_venv=use_venv,
            skip_deps=skip_deps,
            skip_models=skip_models,
            force=force,
            skip_pytorch=not spec.needs_pytorch,
        )
        self.spec = spec

    def run(self) -> bool:
        if not super().run():
            return False

        aliases = list(self.spec.extra_aliases) if self.spec.extra_aliases else None
        self.create_cli_wrappers(extra_aliases=aliases)
        self.create_activate_wrapper()

        self.show_summary(
            commands=[
                f"{self.cli_name} --help",
                f"{self.cli_name} --version" if self.spec.kind == ToolKind.PYTHON else "",
            ],
        )
        return True


class _ToolRustInstaller(RustProjectInstaller):
    """Adaptador: instancia RustProjectInstaller a partir de um ToolSpec."""

    def __init__(
        self,
        spec: ToolSpec,
        monorepo: Path,
        *,
        install_prefix: Path | None = None,
    ) -> None:
        super().__init__(
            project_name=spec.name,
            cli_name=spec.cli_name,
            project_root=spec.project_root(monorepo),
            cargo_bin_name=spec.cargo_bin_name,
            install_prefix=install_prefix,
        )
        self.spec = spec


def install_tool(
    name: str,
    *,
    monorepo: Path | None = None,
    action: str = "install",
    install_prefix: Path | None = None,
    python_cmd: str = "python3",
    use_venv: bool = False,
    skip_deps: bool = False,
    skip_models: bool = False,
    force: bool = False,
) -> bool:
    """Instala/desinstala/reinstala uma ferramenta pelo nome.

    Returns:
        True se a acção teve sucesso.
    """
    if monorepo is None:
        monorepo = find_monorepo_root()

    spec = get_tool(name)

    if not spec.exists(monorepo):
        Logger().error(f"Diretório não encontrado: {spec.project_root(monorepo)}")
        return False

    if spec.kind == ToolKind.PYTHON:
        inst = _ToolPythonInstaller(
            spec,
            monorepo,
            install_prefix=install_prefix,
            python_cmd=python_cmd,
            use_venv=use_venv,
            skip_deps=skip_deps,
            skip_models=skip_models,
            force=force,
        )
    elif spec.kind == ToolKind.RUST:
        inst = _ToolRustInstaller(
            spec,
            monorepo,
            install_prefix=install_prefix,
        )
    else:
        Logger().error(f"Tipo de ferramenta não suportado: {spec.kind}")
        return False

    if action == "install":
        return inst.run()
    elif action == "uninstall":
        if spec.kind == ToolKind.RUST:
            return inst.run_uninstall()
        Logger().warn("Uninstall para Python: pip uninstall <pacote>")
        return True
    elif action == "reinstall":
        if spec.kind == ToolKind.RUST:
            return inst.run_reinstall()
        Logger().warn("Reinstall: use --force com install")
        return install_tool(
            name,
            monorepo=monorepo,
            action="install",
            install_prefix=install_prefix,
            python_cmd=python_cmd,
            use_venv=use_venv,
            skip_deps=skip_deps,
            skip_models=skip_models,
            force=True,
        )
    else:
        Logger().error(f"Acção desconhecida: {action}")
        return False


def install_all(
    *,
    monorepo: Path | None = None,
    install_prefix: Path | None = None,
    python_cmd: str = "python3",
    use_venv: bool = False,
    skip_deps: bool = False,
    skip_models: bool = False,
    force: bool = False,
) -> bool:
    """Instala todas as ferramentas disponíveis no monorepo."""
    if monorepo is None:
        monorepo = find_monorepo_root()

    logger = Logger()
    tools = list_available_tools(monorepo)
    if not tools:
        logger.error("Nenhuma ferramenta encontrada no monorepo.")
        return False

    logger.header(f"Instalando {len(tools)} ferramentas")
    results: dict[str, bool] = {}

    for spec in tools:
        logger.header(f"→ {spec.name}")
        ok = install_tool(
            spec.cli_name,
            monorepo=monorepo,
            install_prefix=install_prefix,
            python_cmd=python_cmd,
            use_venv=use_venv,
            skip_deps=skip_deps,
            skip_models=skip_models,
            force=force,
        )
        results[spec.name] = ok

    logger.header("Resumo")
    for name, ok in results.items():
        status = "OK" if ok else "FALHOU"
        if ok:
            logger.success(f"{name}: {status}")
        else:
            logger.error(f"{name}: {status}")

    return all(results.values())


def main(argv: list[str] | None = None) -> int:
    """Ponto de entrada CLI: ``gamedev-install``."""
    monorepo = find_monorepo_root()
    available = list_available_tools(monorepo)
    tool_names = sorted(TOOLS.keys())

    parser = argparse.ArgumentParser(
        prog="gamedev-install",
        description="Instalador unificado do monorepo GameDev",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=_build_epilog(available),
    )

    parser.add_argument(
        "tool",
        nargs="?",
        default=None,
        help="Ferramenta a instalar (ou 'all' para todas). Opções: "
        + ", ".join(tool_names + ["all"]),
    )
    parser.add_argument(
        "--action",
        choices=["install", "uninstall", "reinstall"],
        default="install",
        help="Acção (default: install)",
    )
    parser.add_argument("--list", action="store_true", help="Listar ferramentas disponíveis")
    parser.add_argument(
        "--prefix",
        default=None,
        help="Prefixo de instalação (default: ~/.local)",
    )
    parser.add_argument("--use-venv", action="store_true", help="Usar .venv existente (Python)")
    parser.add_argument("--skip-deps", action="store_true", help="Não instalar deps de sistema")
    parser.add_argument("--skip-models", action="store_true", help="Não configurar modelos")
    parser.add_argument("--force", action="store_true", help="Forçar reinstalação")
    parser.add_argument(
        "--python",
        default="python3",
        help="Comando Python (default: python3)",
    )

    args = parser.parse_args(argv)
    logger = Logger()

    if args.list:
        _print_tool_list(available, logger)
        return 0

    if args.tool is None:
        parser.print_help()
        return 0

    prefix = Path(args.prefix) if args.prefix else None

    if args.tool.lower() == "all":
        ok = install_all(
            monorepo=monorepo,
            install_prefix=prefix,
            python_cmd=args.python,
            use_venv=args.use_venv,
            skip_deps=args.skip_deps,
            skip_models=args.skip_models,
            force=args.force,
        )
    else:
        ok = install_tool(
            args.tool,
            monorepo=monorepo,
            action=args.action,
            install_prefix=prefix,
            python_cmd=args.python,
            use_venv=args.use_venv,
            skip_deps=args.skip_deps,
            skip_models=args.skip_models,
            force=args.force,
        )

    return 0 if ok else 1


def _build_epilog(available: list[ToolSpec]) -> str:
    lines = [
        "Ferramentas disponíveis:",
        "",
    ]
    for spec in available:
        kind = "Python" if spec.kind == ToolKind.PYTHON else "Rust"
        lines.append(f"  {spec.cli_name:<14s}  [{kind}]  {spec.description}")
    lines.extend([
        "",
        "Exemplos:",
        "  gamedev-install materialize               # Instalar Materialize (Rust)",
        "  gamedev-install text2d --use-venv          # Instalar Text2D no venv",
        "  gamedev-install gameassets --skip-deps      # Instalar GameAssets (sem deps sistema)",
        "  gamedev-install materialize --action uninstall",
        "  gamedev-install all                        # Instalar tudo",
        "  gamedev-install --list                     # Listar ferramentas",
    ])
    return "\n".join(lines)


def _print_tool_list(available: list[ToolSpec], logger: Logger) -> None:
    rows = []
    for spec in available:
        kind = "Python" if spec.kind == ToolKind.PYTHON else "Rust"
        rows.append((f"{spec.cli_name} [{kind}]", spec.description))
    logger.table(rows, title="Ferramentas GameDev disponíveis")


if __name__ == "__main__":
    sys.exit(main())
