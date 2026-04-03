# Resumo da Atualização de Dependências

## Data: 01/04/2026

### Dependências Descobertas Durante Testes do Part3D

O Part3D utiliza modelos da Hugging Face Space do Hunyuan3D-Part que têm dependências adicionais não documentadas explicitamente:

| Dependência | Versão | Onde é Usada | Propósito |
|-------------|--------|--------------|-----------|
| `omegaconf` | >=2.3.0 | X-Part HF Space | Configuração hierárquica YAML |
| `einops` | >=0.8.0 | partformer_dit.py | Operações de rearranjo de tensores (rearrange) |
| `scikit-image` | >=0.24.0 | surface_extractors.py | Extração de superfícies (skimage.measure) |
| `addict` | >=2.4.0 | sonata/model.py | Dicionários auto-acessíveis |
| `spconv-cu121` | >=2.3.0 | sonata/model.py | Sparse convolution para modelos 3D |

### Arquivos Atualizados

1. **Part3D/pyproject.toml**
   - Adicionadas 4 novas dependências
   - Incluído comentários explicativos para cada uma

2. **Paint3D/pyproject.toml**
   - Adicionado `sdnq>=0.1.0` para quantização moderna

3. **Shared/pyproject.toml**
   - Adicionado `sdnq>=0.1.0` na extra `[quantization]`

### Instalação Imediata (venv existente)

```bash
pip install addict
```

### Para Reinstalação Completa (novos ambientes)

```bash
# Part3D
cd Part3D
pip install -e ".[dev]"

# Ou manualmente:
pip install torch numpy trimesh click rich rich-click tqdm Pillow safetensors \
    huggingface-hub diffusers transformers easydict pytorch-lightning pymeshlab \
    optimum-quanto bitsandbytes torchao omegaconf einops scikit-image addict sdnq
```

### Status dos Testes

- ✅ SDNQ disponível no ambiente
- ✅ Dependências instaladas
- 🔄 Testes de quantização em andamento
