# Dependências Atualizadas

## Resumo das alterações em pyproject.toml

### Part3D
Adicionadas dependências descobertas durante testes:
- `omegaconf>=2.3.0` - Necessário para X-Part HF Space
- `einops>=0.8.0` - Necessário para partformer_dit.py (rearrange)
- `sdnq>=0.1.0` - Quantização moderna SDNQ (uint8/int8/int4)

### Paint3D
Adicionada:
- `sdnq>=0.1.0` - Quantização moderna SDNQ para UNet

### Shared
Adicionada às extras `quantization`:
- `sdnq>=0.1.0` - Suporte a quantização SDNQ

## Para instalar nas ferramentas existentes

```bash
# Part3D
cd Part3D
source .venv/bin/activate
pip install omegaconf einops sdnq

# Paint3D
cd Paint3D
source .venv/bin/activate
pip install sdnq

# Shared (já deve estar instalado via Part3D/Paint3D)
cd Shared
pip install -e ".[quantization]"
```

## Testes realizados

- [x] Part3D com SDNQ uint8 - DiT carrega (falta testar geração completa)
- [ ] Part3D baseline (sem quantização)
- [ ] Part3D com quanto-int8
- [ ] Paint3D com SDNQ

## Próximos passos

1. Reinstalar Part3D com novas dependências
2. Continuar testes de quantização
3. Documentar resultados no RESULTADO_OTIMIZACAO.md
