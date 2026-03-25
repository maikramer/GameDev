# gamedev-shared

Biblioteca partilhada do monorepo **GameDev** — código comum entre Text2D, Text3D, GameAssets e Materialize.

## Módulos

| Módulo | Descrição |
|--------|-----------|
| `gamedev_shared.logging` | Logger Rich/ANSI unificado (info, warn, error, step, header, success) |
| `gamedev_shared.cli_rich` | Configuração `rich-click` parametrizada (`setup_rich_click`) |
| `gamedev_shared.skill_install` | Instalação de Agent Skills Cursor genérica por `tool_name` |
| `gamedev_shared.gpu` | Utilitários GPU/memória (format_bytes, get_gpu_info, clear_cuda_memory, ...) |
| `gamedev_shared.subprocess_utils` | Execução de ferramentas via subprocess (resolve_binary, run_cmd, RunResult) |
| `gamedev_shared.env` | Constantes e helpers para variáveis de ambiente do monorepo |
| `gamedev_shared.installer` | Classes base para instaladores (Python e Rust) |
| `gamedev_shared.installer.registry` | Registry de ferramentas do monorepo (ToolSpec, TOOLS, find_monorepo_root) |
| `gamedev_shared.installer.unified` | Instalador unificado — instala qualquer ferramenta (`gamedev-install` CLI) |

## Instalador unificado

Ao instalar o pacote `gamedev-shared`, fica disponível o comando `gamedev-install`:

```bash
gamedev-install --list                     # Listar ferramentas
gamedev-install materialize                # Instalar Materialize (Rust)
gamedev-install text2d --use-venv          # Instalar Text2D no venv
gamedev-install all                        # Instalar tudo
gamedev-install materialize --action uninstall
```

Também pode ser executado sem `pip install` via scripts na raiz do monorepo:

```bash
./install.sh materialize     # Linux/macOS
.\install.ps1 materialize    # Windows PowerShell
```

## Instalação

```bash
# Dentro do monorepo (modo editável)
pip install -e Shared/

# Com suporte GPU
pip install -e "Shared/[gpu]"

# Com CLI (click + rich-click)
pip install -e "Shared/[cli]"
```

## Extras

- `gpu` — torch (para `gamedev_shared.gpu`)
- `cli` — click + rich-click (para `gamedev_shared.cli_rich`)
- `dev` — pytest
