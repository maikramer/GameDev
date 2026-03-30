# Animator3D

CLI de **animação 3D** com [Blender Python API](https://docs.blender.org/api/current/) (`bpy`), pensada para encaixar depois do **Rigging3D** (mesh rigado → keyframes → export GLB/FBX).

## Requisitos

- **Python 3.13** — o wheel PyPI `bpy==5.1.0` exige 3.13 e alinha com **Blender 5.1**.
- Blender embutido no pacote `bpy` (sem abrir janela; execução em background).

## Instalação

### Oficial (monorepo)

Na **raiz** do repositório GameDev (pasta com `install.sh` e `Shared/`):

```bash
cd /caminho/para/GameDev
./install.sh animator3d
```

Equivalente: `gamedev-install animator3d`. Documentação geral: [docs/INSTALLING.md](../docs/INSTALLING.md).

### Manual / desenvolvimento

```bash
cd Animator3D
python3.13 -m venv .venv
source .venv/bin/activate   # Windows: .venv\Scripts\activate
pip install -e ".[dev]"
```

## Uso

```bash
animator3d check
animator3d inspect rigged.glb
animator3d inspect rigged.glb --json-out
animator3d export rigged.glb copia.glb
animator3d wave-idle rigged.glb animado.glb --frames 60
animator3d wave-idle rigged.glb animado.glb --bone mixamorig:Spine
```

| Comando | Descrição |
|---------|-----------|
| `check` | Confirma `bpy` e mostra versão do Blender |
| `inspect` | Importa e lista armatures, amostra de ossos e acções |
| `export` | Re-exporta (teste de roundtrip de import/export) |
| `wave-idle` | Animação de teste (oscilação num osso) e export |

## Fluxo com Rigging3D

1. `rigging3d pipeline --input mesh.glb --output rigged.glb`
2. `animator3d wave-idle rigged.glb com_animacao.glb` (ou pipeline próprio em Python usando `animator3d.bpy_ops`)

## Licença

MIT — ver [`LICENSE`](LICENSE).
