# Materialize CLI — Para agentes de IA

**O que é:** CLI em Rust que gera mapas PBR (Height, Normal, Metallic) a partir de uma imagem de textura difusa, usando compute shaders na GPU (wgpu).

**Quando usar:** Sempre que for preciso gerar mapas PBR a partir de uma textura (ex.: para jogos, rendering 3D, materiais).

**Quando não usar:** Redimensionar imagem, converter formato sem gerar PBR, edição de imagem genérica — use outras ferramentas.

## Sintaxe

```bash
materialize <INPUT> [-o DIR] [-f FORMAT] [-q 0-100] [-v]
```

| Argumento/flag | Obrigatório | Padrão | Descrição |
|----------------|-------------|--------|-----------|
| `INPUT`        | Sim         | —      | Caminho da imagem de entrada (png, jpg, tga, exr) |
| `-o`, `--output` | Não      | `.`    | Diretório de saída |
| `-f`, `--format` | Não      | `png`  | Formato dos arquivos: `png`, `jpg`, `tga`, `exr` |
| `-q`, `--quality` | Não     | `95`   | Qualidade JPEG (0–100), quando `-f jpg` |
| `-v`, `--verbose` | Não     | —      | Saída verbosa (progresso) |

## Exemplos

```bash
# Básico: gera na pasta atual
materialize texture.png

# Saída em diretório específico
materialize texture.png -o ./out/

# Com formato e verbose
materialize brick.png -o ./materials/ -f png -v
```

**Arquivos gerados** (a partir do nome da entrada, ex. `texture.png`):
- `texture_height.png`
- `texture_normal.png`
- `texture_metallic.png`

## Códigos de saída

| Código | Significado |
|--------|-------------|
| `0`    | Sucesso; arquivos gerados (listados em stdout). |
| Não-zero | Erro; mensagem em stderr (ex.: arquivo não encontrado, formato não suportado, falha de GPU). |

Sempre verificar o exit code após invocar; em falha, usar stderr para diagnóstico.

## Documentação completa

- [docs/cli-api.md](docs/cli-api.md) — Referência da CLI
- [docs/README.md](docs/README.md) — Visão geral e instalação
