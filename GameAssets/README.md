# GameAssets

CLI para **batches de prompts e assets** alinhados ao estilo e à ideia do teu jogo. Combina um perfil YAML (`game.yaml`), um manifest CSV e presets de estilo, e orquestra **`text2d`** e **`text3d`** (subprocess).

## Requisitos

- Python 3.10+
- Comandos `text2d` e `text3d` no `PATH` quando fores correr **batch** (instala [Text2D](../Text2D) e [Text3D](../Text3D) nos respetivos ambientes), ou variáveis de ambiente:
  - `TEXT2D_BIN` — caminho absoluto para o executável `text2d`
  - `TEXT3D_BIN` — caminho absoluto para o executável `text3d`
  - `MATERIALIZE_BIN` — opcional; usado pelo Text3D quando activas **PBR** (`text3d.materialize` no `game.yaml`) se o binário `materialize` não estiver no `PATH` (ver [Materialize](../Materialize) e [Text3D: PBR no GLB](../Text3D/docs/PBR_MATERIALIZE.md))

## Instalação (recomendado)

O projeto mantém um **venv** local em `GameAssets/.venv`, como Text2D e Text3D.

```bash
cd GameDev/GameAssets
chmod +x scripts/setup.sh activate.sh
./scripts/setup.sh
source .venv/bin/activate
gameassets --help
```

Opções do script:

| Opção | Efeito |
|--------|--------|
| *(default)* | Cria `.venv` se não existir; `pip install -e .` |
| `--recreate` | Apaga e recria o `.venv` |
| `--dev` | Instala também extras de desenvolvimento (`pytest` via `pip install -e ".[dev]"`) |

**Ativar o ambiente** em cada terminal:

```bash
source /caminho/para/GameDev/GameAssets/.venv/bin/activate
```

O ficheiro `activate.sh` segue o padrão do Text2D: corre um comando já com o venv ativo, por exemplo:

```bash
./activate.sh gameassets prompts --profile game.yaml --manifest manifest.csv
```

**Dependências:** listadas em [`config/requirements.txt`](config/requirements.txt) (instaladas automaticamente pelo `setup.py` ao fazer `pip install -e .`). Desenvolvimento: [`config/requirements-dev.txt`](config/requirements-dev.txt) ou `./scripts/setup.sh --dev`.

## Fluxo em 3 passos

### 1. Inicializar

```bash
gameassets init --path ./meu_jogo
cd meu_jogo
```

Isto cria `game.yaml` (perfil) e `manifest.csv` (lista de assets).

### 2. Rever prompts (sem GPU)

```bash
gameassets prompts --profile game.yaml --manifest manifest.csv
```

Ou gravar JSONL:

```bash
gameassets prompts -o prompts.jsonl --profile game.yaml --manifest manifest.csv
```

### 3. Gerar imagens (e opcionalmente 3D)

Só **2D**:

```bash
gameassets batch --profile game.yaml --manifest manifest.csv
```

**2D + 3D** onde `generate_3d=true` no CSV:

```bash
gameassets batch --profile game.yaml --manifest manifest.csv --with-3d
```

**Preset personalizado** (chave só no teu `presets-local.yaml`, não em `data/presets.yaml`): tens de passar **`--presets-local caminho.yaml`**, caso contrário o comando falha com `Preset desconhecido`.

```bash
gameassets batch --profile game.yaml --manifest manifest.csv --with-3d \
  --presets-local presets-local.yaml --log run.jsonl
```

- Sem `--with-3d`, **nunca** corre `text3d`, mesmo com coluna `generate_3d=true` (apenas aviso).
- `--dry-run` mostra os comandos sem executar.
- `--fail-fast` para no primeiro erro (defeito: continua).
- `--log batch-log.jsonl` acrescenta um JSON por linha processada.
- **Lock exclusivo:** na pasta do manifest é criado `.gameassets_batch.lock` (ficheiro `fcntl`) para **impedir dois batches na mesma pasta** — evita disputa de VRAM entre `text2d`/`text3d` em paralelo. Se o PID no lock já não existir, o lock é recuperado. `--skip-batch-lock` desliga (avançado).
- **VRAM:** antes da execução, se `nvidia-smi` existir e a VRAM livre for inferior a ~1,8 GiB, mostra-se um aviso. `--skip-gpu-preflight` desliga o aviso.
- **CUDA:** os subprocessos `text2d`/`text3d` recebem `PYTORCH_CUDA_ALLOC_CONF=expandable_segments:True` se a variável ainda não estiver definida no ambiente (reduz falhas por fragmentação).

### PBR no GLB (Materialize + Text3D)

Com **`text3d` no `game.yaml`** podes activar o mesmo fluxo documentado em Text3D: após o Hunyuan3D-Paint, o **Materialize CLI** gera normal, oclusão e metallic-roughness e o GLB fica com material glTF completo.

Exemplo mínimo (com `--with-3d` e linhas `generate_3d=true`):

```yaml
text3d:
  preset: fast
  texture: true
  materialize: true
  materialize_save_maps: true
  materialize_maps_subdir: pbr_maps
```

- **`materialize`:** passa `--materialize` ao `text3d generate` (implica textura pintada).
- **`materialize_save_maps`:** se `true`, o Text3D grava mapas PBR num diretório de **staging** (temporário no batch); o GLB fica com material completo.
- **`materialize_export_maps_to_output`:** se `true`, **além** disso copia esses mapas para `output_dir/materialize_maps_subdir/<id>/` (útil para editar texturas no motor). Por defeito `false`, para não encher a pasta do jogo com `pbr_maps/`.
- **`materialize_maps_subdir`:** nome da subpasta sob `output_dir` quando exportas mapas (defeito: `pbr_maps`).
- **`materialize_bin`:** caminho opcional ao binário (equivale a `--materialize-bin`; senão usa `PATH` / `MATERIALIZE_BIN`).
- **`materialize_no_invert`:** se `true`, adiciona `--materialize-no-invert` (roughness sem `1−smoothness`).

Guia completo: [Text3D/docs/PBR_MATERIALIZE.md](../Text3D/docs/PBR_MATERIALIZE.md).

## Perfil (`game.yaml`)

Campos principais:

| Campo | Descrição |
|-------|-----------|
| `title`, `genre`, `tone` | Metadados do perfil; **o título não vai para o prompt de imagem** (evita texto/logótipo no PNG). Género e tom definem ambiente |
| `style_preset` | Chave em `src/gameassets/data/presets.yaml` (`lowpoly`, `pixel_art`, …) |
| `negative_keywords` | Lista extra de restrições (“Avoid: …”) |
| `output_dir` | Raiz de saída (defeito **`.`** → `./images/` e `./meshes/` sem pasta extra `outputs/`) |
| `path_layout` | `split` (defeito) ou `flat` — ver abaixo |
| `images_subdir` / `meshes_subdir` | Usados em `split`: subpastas para PNG/JPG e GLB |
| `image_ext` | `png` ou `jpg` |
| `seed_base` | Opcional; seeds derivados por `id` para reprodutibilidade |
| `text2d` | Bloco opcional: `low_vram`, `cpu`, `width`, `height` |
| `text3d` | Bloco opcional: `preset`, `low_vram`, `texture` (omitido = **`true`**), `steps` / `octree_resolution` / `num_chunks` (alternativa mútua a `preset`), `no_mesh_repair`, `mesh_smooth`, `mc_level`, e **PBR:** `materialize`, `materialize_save_maps`, `materialize_export_maps_to_output`, `materialize_maps_subdir`, `materialize_bin`, `materialize_no_invert` |

### Hunyuan3D e qualidade

Com `text3d.low_vram: true` e GPU CUDA, o **Text3D** envia o Hunyuan3D shape para **CPU** (evita OOM em ~6 GB), mas a **forma** costuma degradar muito (malhas blocosas). Para assets de jogo sérios, usa **`low_vram: false`** com `preset: balanced` ou `fast` na GPU e fecha outras aplicações que usem VRAM (ex.: editor Godot).

### Layout de pastas (`path_layout`)

- **`split`** — `output_dir/images_subdir/<id>.png` e `output_dir/meshes_subdir/<id>.glb`. O `id` pode incluir subpastas (ex. `Props/crate_01`).
- **`flat`** — `output_dir/<dir do id>/<nome>.png` e o mesmo diretório para `<nome>.glb`. Ex.: `id` = `Collectibles/core` → `output_dir/Collectibles/core.png` e `Collectibles/core.glb`. Adequado para **uma pasta por categoria** no Godot, sem ramos separados `images/` e `meshes/`.

Podes criar `presets.local.yaml` ao lado do perfil e passar `--presets-local presets.local.yaml` para fundir presets personalizados.

## Manifest (`manifest.csv`)

Cabeçalhos: **`id`**, **`idea`** (obrigatórios); opcionais: **`kind`** (`prop`, `character`, `environment`), **`generate_3d`** (`true`/`false`/`sim`/…). Com `path_layout: flat`, usa `id` com barra, por exemplo `Crystals/shard_blue`, para gravar ficheiros dentro de `Crystals/`.

## Licença

MIT (alinhado ao resto do monorepo). Os modelos (FLUX, Hunyuan, etc.) têm licenças próprias.
