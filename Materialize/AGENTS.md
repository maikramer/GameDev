# Materialize CLI — Para agentes de IA

**O que é:** CLI em Rust que gera mapas PBR (Height, Normal, Metallic, Smoothness, Edge, AO) a partir de uma imagem de textura difusa, usando compute shaders na GPU (wgpu).

**Quando usar:** Sempre que for preciso gerar mapas PBR a partir de uma textura (ex.: para jogos, rendering 3D, materiais).

**Quando não usar:** Redimensionar imagem, converter formato sem gerar PBR, edição de imagem genérica — use outras ferramentas.

## Sintaxe

```bash
materialize <INPUT> [-o DIR] [-f FORMAT] [-p PRESET] [-q 0-100] [-v]
```

| Argumento/flag | Obrigatório | Padrão | Descrição |
|----------------|-------------|--------|-----------|
| `INPUT`        | Sim         | —      | Caminho da imagem de entrada (png, jpg, tga, exr) |
| `-o`, `--output` | Não      | `.`    | Diretório de saída |
| `-f`, `--format` | Não      | `png`  | Formato dos arquivos: `png`, `jpg`, `tga`, `exr` |
| `-p`, `--preset` | Não      | `default` | Preset de material (ver abaixo) |
| `-q`, `--quality` | Não     | `95`   | Qualidade JPEG (0–100), quando `-f jpg` |
| `-v`, `--verbose` | Não     | —      | Saída verbosa (progresso) |
| `--quiet`      | Não         | —      | Não listar arquivos gerados no sucesso |

## Presets

| Preset | Uso | Características |
|--------|-----|-----------------|
| `default` | Uso geral | Configurações balanceadas para qualquer textura |
| `skin` | Pele humana/personagem | Metallic zero, smoothness médio-alto, normals suaves |
| `floor` | Chão/piso (pedra, terra, azulejo) | Height pronunciado, AO forte, superfície áspera |
| `metal` | Superfícies metálicas | Metallic boost, edges nítidos, polido |
| `fabric` | Tecido/roupa | Matte, sem metallic, edges suaves |
| `wood` | Madeira | Sem metallic, detalhe de grão moderado |
| `stone` | Pedra/rocha | Muito áspero, AO profundo, normals fortes |

## Exemplos

```bash
# Básico: gera na pasta atual com preset default
materialize texture.png

# Textura de pele humana
materialize skin_diffuse.png -p skin -o ./out/

# Chão de pedra
materialize stone_floor.png --preset floor -o ./materials/ -v

# Superfície metálica em JPG
materialize metal_plate.png -p metal -f jpg -q 95

# Saída em diretório específico
materialize texture.png -o ./out/
```

**Arquivos gerados** (a partir do nome da entrada, ex. `texture.png`):
- `texture_height.png`
- `texture_normal.png`
- `texture_metallic.png`
- `texture_smoothness.png`
- `texture_edge.png`
- `texture_ao.png`

## Códigos de saída

| Código | Significado |
|--------|-------------|
| `0`    | Sucesso; arquivos gerados (listados em stdout). |
| Não-zero | Erro; mensagem em stderr (ex.: arquivo não encontrado, formato não suportado, falha de GPU). |

Sempre verificar o exit code após invocar; em falha, usar stderr para diagnóstico.

## Documentação completa

- [docs/cli-api.md](docs/cli-api.md) — Referência da CLI
- [docs/README.md](docs/README.md) — Visão geral e instalação
