# Software de terceiros

## UniRig

`src/rigging3d/unirig/` contém código do [UniRig](https://github.com/VAST-AI-Research/UniRig) (MIT).

- Paper: [One Model to Rig Them All](https://arxiv.org/abs/2504.12451)
- Pesos: [VAST-AI/UniRig](https://huggingface.co/VAST-AI/UniRig)
- Licença: `src/rigging3d/unirig/LICENSE`

Incluídos apenas ficheiros de **inferência**; treino e extras no repositório upstream.

O `run.py` é uma versão simplificada (só predict) do entrypoint original.

Patches: `unirig/src/inference/merge.py` — removido `import argparse` duplicado.
