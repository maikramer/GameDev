# GameDevLab

CLI de **laboratório** do monorepo GameDev: debug 3D (Animator3D), bancadas de quantização (Part3D, Paint3D, SDNQ, Quanto, etc.), pré-quantização e profiling. Substitui o antigo `gameassets debug` (removido do GameAssets).

## Instalação

A partir da raiz do repositório (ou dentro de `GameDevLab/`):

```bash
cd GameDev/GameDevLab
pip install -e .
# Bancadas que importam Part3D/Paint3D/torch:
pip install -e ".[bench]"
```

Sem `pip install`, apenas com `PYTHONPATH` (requer `Shared` no path):

```bash
export PYTHONPATH="/path/to/GameDev/GameDevLab/src:/path/to/GameDev/Shared/src"
python -m gamedev_lab --help
```

Variável opcional: `GAMEDEV_ROOT` — raiz do monorepo (por defeito inferida a partir da localização do pacote).

## Comandos

| Grupo | Uso |
|--------|-----|
| `gamedev-lab check glb` | Valida um GLB contra regras YAML/JSON (inspect; exit 0/1 para CI) |
| `gamedev-lab debug` | `screenshot`, `bundle`, `inspect`, `inspect-rig`, `compare` — requer `animator3d` ou `ANIMATOR3D_BIN` |
| `gamedev-lab bench part3d` | Testes de decomposição com VRAM (`--mesh`, `--modo`, `--project-dir`) |
| `gamedev-lab bench paint-vram` | Sweet spot de quantização Paint3D (extra `[bench]`, GPU) |
| `gamedev-lab bench pre-quantize` | Pré-quantização SDNQ DiT/UNet |
| `gamedev-lab bench sdnq-sweep` | Varre configs SDNQ otimizadas (4-bit, 8-bit, TinyVAE) |
| `gamedev-lab bench pipeline-opt` | Otimiza pipeline Part3D+Paint3D com fallback automático em OOM |
| `gamedev-lab bench batch` | Varre configs e chama `gameassets batch` (`GAMEASSETS_BIN` / PATH) |
| `gamedev-lab profile cprofile` | `python -m cProfile` sobre um script |

Exemplos:

```bash
gamedev-lab debug bundle modelo.glb -o ./out_bundle
gamedev-lab bench part3d --project-dir GameAssets/examples/batch_realista_colorido \
  --mesh meshes/boa_mesa/tigela_ceramica.glb --modo sdnq-uint8

# Otimização SDNQ avançada (TinyVAE, attention slicing, 4/8-bit)
gamedev-lab bench sdnq-sweep --mesh modelo.glb --image ref.png \
  --target-vram-mb 5500 --output-dir sweep_results

# Pipeline completo com otimização automática (sem OOM)
gamedev-lab bench pipeline-opt --mesh input.glb --image ref.png \
  --target-vram-mb 6000 --steps 50 --octree 256

gamedev-lab profile cprofile -o run.prof ./script.py -- arg1 arg2
```

## Comparação e validação

- **`gamedev-lab check glb modelo.glb regras.yaml`** — avalia limites de vértices/faces, `world_bounds`, ossos obrigatórios, etc. Ver [`examples/glb_rules.example.yaml`](examples/glb_rules.example.yaml).
- **`gamedev-lab debug compare a.glb b.glb`** — side-by-side por vista; **`--struct-diff`** (defeito) gera `inspect_diff` no `diff_report.json`; **`--image-metrics`** adiciona MAE/RMSE/SSIM; **`--fail-below-ssim 0.85`** falha se alguma vista estiver abaixo (com `--image-metrics`).
- **`gamedev-lab debug inspect-rig`** — delega a `animator3d inspect-rig` (ossos + heatmap opcional). **`debug bundle --include-rig`** gera subpasta `rig/`.

Renderização: flags **`--engine workbench|eevee`**, **`--ortho`**, **`--no-transparent-film`** em `screenshot`, `compare`, `bundle` e `inspect-rig` (repasse ao Animator3D).

## Migração desde GameAssets

- Antes: `gameassets debug bundle …`
- Agora: `gamedev-lab debug bundle …` (o campo `bundle.json` usa `tool: gamedev_lab.debug.bundle`).

## Otimização SDNQ Avançada

O `gamedev-lab bench sdnq-sweep` testa automaticamente múltiplas configurações de quantização SDNQ para encontrar a melhor combinação de qualidade e uso de VRAM.

### Configurações Recomendadas (Estáveis)

Estas configs usam a **quantização nativa qint8 do Paint3D** (não SDNQ), que é estável e testada:

| Configuração | Quantização | TinyVAE | Views | Resolução | VRAM | Estado |
|-------------|-------------|---------|-------|-----------|------|--------|
| `paint3d-qint8-balanced` | qint8 nativo | Não | 6 | 384px | Médio | **Estável** |
| `paint3d-qint8-stable` | qint8 nativo | Não | 4 | 256px | Baixo | **Estável** |

### Configurações SDNQ (Experimental)

⚠️ **Atenção**: As configurações SDNQ são experimentais e requerem validação adicional no Paint3D:

| Configuração | Bits | TinyVAE | Views | Resolução | Uso de VRAM |
|-------------|------|---------|-------|-----------|-------------|
| `sdnq-uint8-full` | 8 | Não | 6 | 512px | Alto |
| `sdnq-uint8-tiny` | 8 | Sim | 4 | 384px | Médio |
| `sdnq-uint8-minimal` | 8 | Sim | 2 | 256px | Baixo |
| `sdnq-int4-full` | 4 | Não | 6 | 512px | Médio |
| `sdnq-int4-tiny` | 4 | Sim | 4 | 384px | Baixo |
| `sdnq-int4-minimal` | 4 | Sim | 2 | 256px | Mínimo |
| `sdnq-fp8` | 8 (FP8) | Não | 6 | 512px | Alto (RTX 40 series) |

**Notas sobre compatibilidade:**
- **TinyVAE**: Incompatível com `HunyuanPaintPBR` (requer `latent_dist` que TinyVAE não fornece)
- **SDNQ no Paint3D**: O UNet customizado `UNet2p5DConditionModel` pode ter comportamento diferente com SDNQ aplicado
- **Part3D com SDNQ**: Funciona corretamente com `sdnq-uint8`

### Técnicas de Otimização

- **TinyVAE (TAESD)**: Reduz uso de VRAM do VAE em ~70% (⚠️ incompatível com HunyuanPaintPBR)
- **Attention Slicing**: Processa attention em slices para reduzir pico de VRAM
- **VAE Tiling**: Processa imagens grandes em tiles
- **torch.compile**: Acelera inferência com compilação JIT (⚠️ pode causar instabilidade)
- **Fallback Automático**: Em caso de OOM, automaticamente tenta configuração mais conservadora

### Pipeline Otimizado

O `gamedev-lab bench pipeline-opt` executa o pipeline completo (Part3D → Paint3D) testando diferentes combinações de quantização para encontrar a que funciona sem OOM no seu hardware.

```bash
# Encontrar melhor config para GPU 6GB
gamedev-lab bench pipeline-opt \
  --mesh input.glb \
  --image reference.png \
  --target-vram-mb 5500 \
  --steps 50 \
  --octree 256
```

O sistema automaticamente:
1. Testa **configs Paint3D estáveis primeiro** (`paint3d-qint8-*`)
2. Depois testa configs SDNQ (experimental)
3. Testa Part3D com diferentes modos de quantização
4. Monitora VRAM em tempo real
5. Em caso de OOM, faz fallback para configuração mais leve
6. Recomenda a melhor combinação encontrada
