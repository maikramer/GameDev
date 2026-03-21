# Text2D

CLI de **text-to-imagem** com [FLUX.2 Klein 4B](https://huggingface.co/black-forest-labs/FLUX.2-klein-4B) em quantização **SDNQ** ([Disty0](https://huggingface.co/Disty0/FLUX.2-klein-4B-SDNQ-4bit-dynamic)), no mesmo espírito do Text3D (Click + Rich, `src/`, scripts).

## Requisitos

| Item    | Mínimo | Notas |
|---------|--------|--------|
| Python  | 3.10+  | Testado em 3.10–3.13 |
| GPU     | Opcional | NVIDIA + CUDA recomendado para inferência razoável |
| VRAM    | ~6 GB+ com `--low-vram` e 512² | Depende do checkpoint; GPUs modestas: `--low-vram` |
| Disco   | ~8 GB  | Cache HF + pesos SDNQ (~2,5 GB em disco) |

**Licença dos pesos:** consulte o [model card Disty0](https://huggingface.co/Disty0/FLUX.2-klein-4B-SDNQ-4bit-dynamic) (pode diferir do Apache 2.0 do modelo base BFL).

## Instalação rápida

```bash
cd Text2D
chmod +x scripts/setup.sh
./scripts/setup.sh
source .venv/bin/activate
text2d --help
```

- Com **NVIDIA**, `setup.sh` instala PyTorch com CUDA (em **Python 3.13+** usa wheels do **PyPI**; em 3.10–3.12 usa o índice `cu121`/`cu118`).
- Dependências de runtime: [`config/requirements.txt`](config/requirements.txt). Desenvolvimento/testes: [`config/requirements-dev.txt`](config/requirements-dev.txt) ou `pip install -e ".[dev]"`.

### Instalador (paridade Text3D)

Com `.venv` já criado (ex.: após `setup.sh`):

```bash
chmod +x scripts/install.sh
./scripts/install.sh --use-venv --prefix ~/.local
```

Instalação completa no `python3` do sistema (PyTorch + requirements + pacote + wrappers em `PREFIX/bin`):

```bash
python3 scripts/installer.py --prefix ~/.local
```

Opções: `--use-venv`, `--skip-deps`, `--skip-models`, `--force`, `--prefix`, `--python`. Sem `.venv` e com `--use-venv`, o instalador **termina com erro** (crie o venv primeiro).

Documentação detalhada: [docs/INSTALL.md](docs/INSTALL.md). Problemas de GPU/carregamento: [docs/TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md).

## Primeira geração vs seguintes

- **1.ª execução:** descarga de vários GB do Hugging Face — pode levar **vários minutos**; a GPU pode mostrar **0%** durante rede/disco (normal).
- **Com cache local:** o mesmo comando costuma ficar na ordem de **segundos a ~1 min** (carregar do disco + inferência), conforme hardware.

## Uso

```bash
text2d generate "um gato com um cartaz que diz olá mundo"

text2d generate "paisagem ao pôr do sol" --width 768 --height 768 --steps 4 --guidance 1.0

text2d generate "retrato" --low-vram -o minha.png --seed 42

text2d generate "teste" -v          # --verbose no próprio subcomando
text2d -v generate "teste"          # ou verbose no grupo

text2d info
text2d models
```

### Variáveis de ambiente

- `TEXT2D_MODEL_ID` — outro repositório HF compatível com `Flux2KleinPipeline`.
- `HF_HOME` — cache Hugging Face (predefinido `~/.cache/huggingface`).

### Guidance

O checkpoint **SDNQ Disty0** usa por defeito **guidance 1.0** (ver [card do modelo](https://huggingface.co/Disty0/FLUX.2-klein-4B-SDNQ-4bit-dynamic)). O BF16 original BFL costuma usar valores mais altos (ex. 4.0).

## GGUF / Unsloth

Pesos **GGUF** destinam-se a fluxos **ComfyUI-GGUF**, não a este CLI (Diffusers).

## Desenvolvimento

```bash
pip install -e ".[dev]"
pytest tests/ -v
```

## Licença

Código do projeto: MIT — [LICENSE](LICENSE). Pesos e termos de uso: repositórios Hugging Face respetivos.
