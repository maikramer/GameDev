"""Templates YAML/CSV para gameassets init."""

GAME_YAML = """# Perfil do jogo — edita os campos e escolhe style_preset (ver GameAssets/src/gameassets/data/presets.yaml)

title: "Meu Jogo"
genre: "roguelike de ação"
tone: "colorido e acessível"

# Um dos presets embutidos: lowpoly, pixel_art, painterly, realistic_stylized
# Se usares um preset só no teu presets-local.yaml, o batch DEVE incluir --presets-local ficheiro.yaml
style_preset: lowpoly

# Palavras ou frases a evitar (além do preset)
negative_keywords:
  - "gore explícito"
  - "logotipos de marcas"

# Raiz dos assets no projeto do jogo (defeito: diretório atual → ./images e ./meshes, sem pasta outputs/)
output_dir: .
# split: images_subdir/ + meshes_subdir/ (cada id pode ter subpastas, ex. Cat/item.png)
# flat: PNG e GLB na mesma pasta por categoria — id recomendado "Categoria/nome" → output_dir/Categoria/nome.png e .glb
path_layout: split
images_subdir: images
meshes_subdir: meshes
image_ext: png

# Opcional: seed base para reprodutibilidade (inteiro)
# seed_base: 42

# Fonte de imagens 2D: text2d (defeito, FLUX Klein) ou texture2d (texturas seamless HF).
# image_source: text2d

# Text2D em batch: low_vram=true recomendado em GPUs ~6 GB (CPU offload do FLUX).
# Se der OOM, fecha o Godot/outros que usem a GPU antes do batch; em último caso cpu: true (lento).
text2d:
  low_vram: true
  width: 768
  height: 768

# Texture2D (HF API) — descomenta image_source + bloco abaixo para seamless em vez de Text2D.
# Combo PBR: materialize: true gera mapas (normal, height, metallic) com o CLI materialize.
# image_source: texture2d
# texture2d:
#   width: 1024
#   height: 1024
#   materialize: true
#   materialize_maps_subdir: pbr_maps

# Skymap2D (HF API) — descomenta image_source + bloco abaixo para skymaps equirectangular 360°.
# image_source: skymap2d
# skymap2d:
#   width: 2048
#   height: 1024

# Text2Sound com generate_audio no CSV: áudio por linha (Stable Audio Open); audio_subdir + bloco opcional.
# audio_subdir: audio
# text2sound:
#   duration: 10
#   steps: 100
#   cfg_scale: 4.5
#   audio_format: wav

# Text3D com --with-3d: preset fast equilibra tempo/VRAM; texture=true activa Paint.
# low_vram=true no text3d => Hunyuan *shape* em CPU (forma muito pior — evita salvo último recurso).
# phased_batch: true (com texture) => batch em 3 passos: shape (todos) → Paint (todos) → materialize-pbr (todos).
text3d:
  preset: fast
  low_vram: false
  texture: true
  # phased_batch: false
  # materialize: true
  # materialize_save_maps: true
  # materialize_export_maps_to_output: false
  # materialize_maps_subdir: pbr_maps
"""

MANIFEST_CSV = """id,idea,kind,generate_3d,generate_audio,image_source
chest_01,"baú de madeira com ferrolhos dourados",prop,false,false,
hero_sword,"espada longa com gema azul no punho",prop,true,false,
forest_bg,"floresta densa ao entardecer",environment,false,false,
sky_sunset,"pôr do sol sobre montanhas, nuvens douradas",skymap,false,false,skymap2d
"""
