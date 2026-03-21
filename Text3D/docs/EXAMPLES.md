# Exemplos - Text3D

Coleção de exemplos avançados e casos de uso.

## Índice

- [Exemplos Básicos](#exemplos-básicos)
- [Geração em Lote](#geração-em-lote)
- [Variações de Prompt](#variações-de-prompt)
- [Pós-Processamento](#pós-processamento)
- [Integrações](#integrações)
- [Automação](#automação)

## Exemplos Básicos

### Geração Simples

```bash
# Robô
text3d generate "a friendly robot with wheels" --output robot.glb

# Carro
text3d generate "a red sports car with black wheels" --output car.glb

# Móveis
text3d generate "a wooden dining table" --output table.glb
text3d generate "an office chair with wheels" --output chair.glb

# Natureza
text3d generate "a potted cactus" --output cactus.glb
text3d generate "a pine tree" --output tree.glb
```

### Diferentes Qualidades

```bash
# Rápido (para prototipagem)
text3d generate "a sword" --steps 16 --frame-size 128 --output sword_quick.glb

# Padrão (equilíbrio)
text3d generate "a sword" --steps 32 --frame-size 256 --output sword.glb

# Alta Qualidade
text3d generate "a sword" --steps 64 --frame-size 512 --guidance 20.0 --output sword_hq.glb
```

### Com Preview (GIF)

```bash
# Gerar mesh + preview animado
text3d generate "a treasure chest" --gif --output chest.glb

# Resultado:
# - chest.glb (mesh 3D)
# - chest_preview.gif (animação rotacionando)
```

## Geração em Lote

### Script de Batch

```python
#!/usr/bin/env python3
"""Batch generation script"""
from text3d import ShapEGenerator
from text3d.utils import save_mesh
import os

# Lista de objetos para gerar
objects = [
    "a wooden crate",
    "a metal barrel",
    "a stone pillar",
    "a futuristic crate",
    "a rusty barrel",
    "an ancient pillar"
]

# Configuração
output_dir = "outputs/batch"
os.makedirs(output_dir, exist_ok=True)

with ShapEGenerator() as gen:
    for i, obj in enumerate(objects, 1):
        print(f"[{i}/{len(objects)}] Generating: {obj}")
        
        mesh = gen.generate(
            prompt=obj,
            num_inference_steps=32,
            guidance_scale=15.0
        )
        
        filename = obj.replace(" ", "_") + ".glb"
        filepath = os.path.join(output_dir, filename)
        save_mesh(mesh, filepath, format="glb")
        
        print(f"  ✓ Saved: {filepath}")

print(f"\nBatch complete! Generated {len(objects)} models.")
```

### Execução

```bash
chmod +x batch_generate.py
python3 batch_generate.py
```

## Variações de Prompt

### Teste A/B de Prompts

```python
"""Testar diferentes descrições do mesmo objeto"""
from text3d import ShapEGenerator
from text3d.utils import save_mesh

prompts = [
    "a sword",
    "a medieval sword",
    "a shiny medieval sword with ornate hilt",
    "a longsword with silver blade and golden hilt",
    "a two-handed greatsword with intricate engravings"
]

with ShapEGenerator() as gen:
    for i, prompt in enumerate(prompts):
        mesh = gen.generate(prompt, num_inference_steps=32)
        save_mesh(mesh, f"sword_variant_{i}.glb", format="glb")
```

### Variações de Estilo

```bash
# Mesmo objeto, estilos diferentes
text3d generate "a chair" --output chair_default.glb
text3d generate "a wooden chair" --output chair_wooden.glb
text3d generate "a modern minimalist chair" --output chair_modern.glb
text3d generate "a medieval throne" --output chair_medieval.glb
text3d generate "a futuristic floating chair" --output chair_futuristic.glb
```

### Variações de Cor

```bash
# Mesmo objeto, cores diferentes
text3d generate "a red sports car" --output car_red.glb
text3d generate "a blue sports car" --output car_blue.glb
text3d generate "a black sports car" --output car_black.glb
text3d generate "a white sports car" --output car_white.glb
```

## Pós-Processamento

### Combinar Meshes

```python
"""Combinar múltiplos meshes em uma cena"""
import trimesh
from text3d import ShapEGenerator
from text3d.utils import save_mesh

with ShapEGenerator() as gen:
    # Gerar partes
    table = gen.generate("a wooden table")
    chair1 = gen.generate("a wooden chair")
    chair2 = gen.generate("a wooden chair")
    
    # Posicionar
    chair1 = chair1.apply_translation([1.5, 0, 0])
    chair2 = chair2.apply_translation([-1.5, 0, 0])
    
    # Combinar
    scene = trimesh.util.concatenate([table, chair1, chair2])
    
    # Salvar cena
    save_mesh(scene, "dining_set.glb", format="glb")
```

### Redimensionar e Normalizar

```python
"""Normalizar tamanho de meshes para game assets"""
from text3d.utils import get_mesh_info
import numpy as np

def normalize_mesh(mesh, target_height=2.0):
    """Escala mesh para altura alvo"""
    info = get_mesh_info(mesh)
    height = info['bounds'][1][1] - info['bounds'][0][1]  # Y axis
    scale = target_height / height
    return mesh.apply_scale(scale)

# Uso
mesh = gen.generate("a character")
mesh_normalized = normalize_mesh(mesh, target_height=1.8)  # 1.8m person
save_mesh(mesh_normalized, "character_180cm.glb")
```

### Otimização de Mesh

```python
"""Otimizar mesh para games"""
import trimesh

def optimize_for_game(mesh, target_faces=1000):
    """Simplificar mesh para performance"""
    # Simplificação
    mesh_simplified = mesh.simplify_quadric_decimation(target_faces)
    
    # Remover duplicatas
    mesh_simplified.merge_vertices()
    
    # Remover faces degeneradas
    mesh_simplified.remove_degenerate_faces()
    
    # Remover componentes não-usados
    mesh_simplified.remove_unreferenced_vertices()
    
    return mesh_simplified

# Uso
mesh = gen.generate("a complex statue")
mesh_optimized = optimize_for_game(mesh, target_faces=500)
save_mesh(mesh_optimized, "statue_optimized.glb")
```

## Integrações

### Unity

```python
"""Exportar para Unity"""
# Unity suporta GLB nativamente (via GLTFast package)
# OU usar OBJ como fallback

from text3d.utils import save_mesh

# GLB é recomendado (materiais inclusos)
save_mesh(mesh, "export_for_unity.glb", format="glb", rotate=True)
```

### Blender

```bash
# Gerar e importar no Blender

# 1. Gerar modelo
text3d generate "a vase" --output vase.glb

# 2. No Blender:
# File > Import > glTF 2.0 (.glb/.gltf)
# Selecionar vase.glb
```

```python
"""Script Python para Blender"""
import bpy

# Importar GLB
bpy.ops.import_scene.gltf(filepath="/path/to/vase.glb")

# Acessar objeto importado
obj = bpy.context.selected_objects[0]

# Aplicar modificadores
modifier = obj.modifiers.new(name="Decimate", type='DECIMATE')
modifier.ratio = 0.5  # Reduzir 50%

# Exportar
bpy.ops.export_scene.obj(filepath="/path/to/vase_optimized.obj")
```

### Unreal Engine

```bash
# GLB suportado nativamente no UE 4.24+
# Ou usar Datasmith para melhor fluxo

# 1. Gerar
text3d generate "a weapon" --output weapon.glb

# 2. No Unreal:
# Importar como Static Mesh
# Configurar materiais manualmente
```

### Godot

```bash
# Godot 4.0+ suporta GLB nativamente

text3d generate "a pickup item" --output pickup.glb

# No Godot:
# Arrastar pickup.glb para cena
# Adicionar collider manualmente
```

## Automação

### Geração sob Demanda

```python
#!/usr/bin/env python3
"""Serviço simples de geração"""
from text3d import ShapEGenerator
from text3d.utils import save_mesh
import sys

def generate_asset(prompt, output_path, quality="medium"):
    """Gerar asset com qualidade configurável"""
    
    configs = {
        "low": {"steps": 16, "guidance": 10.0, "frame_size": 128},
        "medium": {"steps": 32, "guidance": 15.0, "frame_size": 256},
        "high": {"steps": 64, "guidance": 20.0, "frame_size": 512}
    }
    
    config = configs.get(quality, configs["medium"])
    
    with ShapEGenerator() as gen:
        mesh = gen.generate(prompt, **config)
        save_mesh(mesh, output_path, format="glb")
        
    return output_path

# CLI
if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("Usage: python generate_service.py <prompt> <output> [quality]")
        sys.exit(1)
    
    prompt = sys.argv[1]
    output = sys.argv[2]
    quality = sys.argv[3] if len(sys.argv) > 3 else "medium"
    
    result = generate_asset(prompt, output, quality)
    print(f"Generated: {result}")
```

### Uso

```bash
python generate_service.py "a magic potion" potion.glb high
```

### Pipeline CI/CD

```yaml
# .github/workflows/generate-assets.yml
name: Generate 3D Assets

on:
  workflow_dispatch:
    inputs:
      prompt:
        description: 'Asset description'
        required: true

jobs:
  generate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Setup Python
        uses: actions/setup-python@v4
        with:
          python-version: '3.10'
      
      - name: Install
        run: |
          pip install -e .
      
      - name: Generate
        run: |
          python generate_service.py "${{ github.event.inputs.prompt }}" output.glb
      
      - name: Upload
        uses: actions/upload-artifact@v3
        with:
          name: generated-asset
          path: output.glb
```

## Exemplos por Categoria

### Props para Games

```bash
# RPG
for item in "a health potion bottle" "a mana crystal" "a treasure chest" "a wooden barrel"; do
    filename=$(echo "$item" | tr ' ' '_')
    text3d generate "$item" --output "rpg_${filename}.glb"
done

# FPS
for item in "an ammo box" "a medkit" "a weapon crate" "a tactical shield"; do
    filename=$(echo "$item" | tr ' ' '_')
    text3d generate "$item" --output "fps_${filename}.glb"
done

# Sci-Fi
for item in "a holographic display" "a space crate" "a control panel" "a laser turret"; do
    filename=$(echo "$item" | tr ' _')
    text3d generate "$item" --output "scifi_${filename}.glb"
done
```

### Móveis

```bash
# Conjunto de sala
text3d generate "a modern sofa" --output furniture/sofa.glb
text3d generate "a coffee table" --output furniture/coffee_table.glb
text3d generate "a floor lamp" --output furniture/lamp.glb
text3d generate "a tv stand" --output furniture/tv_stand.glb
text3d generate "a bookshelf" --output furniture/bookshelf.glb
```

### Veículos

```bash
# Carros
text3d generate "a compact car" --output vehicles/compact.glb
text3d generate "a pickup truck" --output vehicles/pickup.glb
text3d generate "a sports car" --output vehicles/sports.glb

# Futuristas
text3d generate "a flying car" --output vehicles/flying_car.glb
text3d generate "a hover bike" --output vehicles/hover_bike.glb
```

### Arquitetura

```bash
# Elementos arquitetônicos
text3d generate "a gothic arch" --output architecture/arch.glb
text3d generate "a stone column" --output architecture/column.glb
text3d generate "a modern staircase" --output architecture/stairs.glb
text3d generate "a wooden door frame" --output architecture/door_frame.glb
```

### Natureza

```bash
# Vegetação
text3d generate "an oak tree" --output nature/oak_tree.glb
text3d generate "a pine tree" --output nature/pine_tree.glb
text3d generate "a palm tree" --output nature/palm_tree.glb
text3d generate "a large rock" --output nature/rock.glb
text3d generate "a bush" --output nature/bush.glb
```

## Dicas de Prompt

### Estrutura Efetiva

```
[Adjetivos] + [Objeto] + [Detalhes] + [Material] + [Estilo]

Exemplos:
- "a weathered wooden treasure chest with iron bands"
- "a sleek futuristic drone with glowing blue lights"
- "an ancient stone pillar with moss and cracks"
- "a polished crystal sphere on a metal stand"
```

### Cores

```bash
# Cores específicas funcionam bem
text3d generate "a red sword"  # Simples
text3d generate "a crimson sword with silver hilt"  # Descritivo

# Múltiplas cores
text3d generate "a blue and yellow striped shield"
```

### Materiais

```bash
# Materiais específicos ajudam
text3d generate "a metal barrel"  # vs apenas "barrel"
text3d generate "a wooden crate"  # vs apenas "crate"
text3d generate "a glass bottle"  # Transparência (limitada)
```

### Tamanho/Proporção

```bash
# Tamanho implícito
text3d generate "a small key"  # vs "a giant key"
text3d generate "a massive dragon statue"  # vs "a small figurine"
```

## Recursos Adicionais

- [Shap-E Gallery](https://huggingface.co/openai/shap-e) - Exemplos oficiais
- [Diffusers Docs](https://huggingface.co/docs/diffusers/en/using-diffusers/shap-e) - Documentação técnica
- [Prompt Engineering Guide](https://promptingguide.ai/) - Técnicas de prompts
