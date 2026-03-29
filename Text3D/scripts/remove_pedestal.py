#!/usr/bin/env python3
"""
Remove pedestal/plataforma de arquivos GLB/OBJ.

Útil para pós-processar modelos onde a sombra do 2D virou um pedestal
grudado nos pés do personagem.

Exemplos:
    python scripts/remove_pedestal.py modelo.glb --output modelo_limpo.glb
    python scripts/remove_pedestal.py modelo.glb --very-aggressive
    python scripts/remove_pedestal.py modelo.obj --output modelo.glb --remesh
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import Literal


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Remove pedestal/plataforma da base de modelos 3D",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Modos:
  defeito          — conservador, só remove componentes desconexas e faces horizontais na base
  --aggressive     — modo forte, adiciona cilindro de corte na base
  --very-aggressive — modo EXTREMO, usa flood-fill para pedestais conectados ao modelo

Exemplos:
  %(prog)s dragao.glb --output dragao_limpo.glb
  %(prog)s dragao.glb --very-aggressive --fill-holes 24
  %(prog)s modelo.obj --aggressive --remesh --remesh-resolution 200
        """,
    )
    parser.add_argument("input", type=Path, help="Arquivo de entrada (.glb, .gltf, .obj, .fbx)")
    parser.add_argument("-o", "--output", type=Path, help="Arquivo de saída (defeito: input_clean.glb)")
    parser.add_argument(
        "--aggressive",
        action="store_true",
        help="Modo agressivo — cilindro na base + peel mais forte",
    )
    parser.add_argument(
        "--very-aggressive",
        action="store_true",
        help="Modo EXTREMO — flood-fill + análise de silhueta para pedestais grudados",
    )
    parser.add_argument(
        "--keep-largest",
        action="store_true",
        default=True,
        help="Mantém só a maior componente conexa (defeito: True)",
    )
    parser.add_argument(
        "--fill-holes",
        type=int,
        default=16,
        help="Fecha buracos com até N arestas (defeito: 16, 0 = desliga)",
    )
    parser.add_argument(
        "--remesh",
        action="store_true",
        help="Isotropic remeshing — reconstrói topologia com triângulos uniformes",
    )
    parser.add_argument(
        "--remesh-resolution",
        type=int,
        default=150,
        help="Resolução do remeshing (~subdivisões na diagonal)",
    )
    parser.add_argument(
        "--smooth",
        type=int,
        default=0,
        help="Suavização Laplaciana (1-2 reduz aspereza)",
    )
    parser.add_argument(
        "--mesh-space",
        choices=["hunyuan", "y_up"],
        default="y_up",
        help="Espaço da mesh: hunyuan (Text3D bruto) ou y_up (GLB orientado)",
    )

    args = parser.parse_args()

    try:
        import trimesh as tm
    except ImportError:
        print("Erro: trimesh não instalado. Instale com: pip install trimesh", file=sys.stderr)
        return 1

    # Importa do Text3D
    try:
        from text3d.utils.mesh_repair import repair_mesh
    except ImportError:
        # Tenta importar diretamente do source
        sys.path.insert(0, str(Path(__file__).parent.parent / "src"))
        from text3d.utils.mesh_repair import repair_mesh

    input_path = Path(args.input)
    if not input_path.exists():
        print(f"Erro: arquivo não encontrado: {input_path}", file=sys.stderr)
        return 1

    # Output padrão
    if args.output:
        output_path = Path(args.output)
    else:
        stem = input_path.stem
        suffix = input_path.suffix
        if suffix in (".glb", ".gltf"):
            output_path = input_path.parent / f"{stem}_clean.glb"
        else:
            output_path = input_path.parent / f"{stem}_clean{suffix}"

    print(f"Carregando: {input_path}")
    try:
        mesh = tm.load(str(input_path), force="mesh")
    except Exception as e:
        print(f"Erro ao carregar mesh: {e}", file=sys.stderr)
        return 1

    print(f"  Vértices: {len(mesh.vertices):,}")
    print(f"  Faces: {len(mesh.faces):,}")

    print("\nReparando mesh...")
    mode_str = ""
    if args.very_aggressive:
        mode_str = " (modo EXTREMO)"
    elif args.aggressive:
        mode_str = " (modo agressivo)"
    print(f"  - Anti-sombra{mode_str}")
    if args.keep_largest:
        print("  - Manter maior componente")
    if args.fill_holes > 0:
        print(f"  - Fechar buracos (max {args.fill_holes} arestas)")
    if args.remesh:
        print(f"  - Remeshing (res={args.remesh_resolution})")
    if args.smooth > 0:
        print(f"  - Suavização ({args.smooth} iters)")

    try:
        cleaned = repair_mesh(
            mesh,
            keep_largest=args.keep_largest,
            merge_vertices=True,
            remove_ground_shadow=True,
            ground_artifact_mesh_space=args.mesh_space,
            ground_shadow_aggressive=args.aggressive and not args.very_aggressive,
            ground_shadow_very_aggressive=args.very_aggressive,
            remove_small_island_fragments=True,
            fill_small_holes_max_edges=args.fill_holes,
            smooth_iterations=args.smooth,
            remesh=args.remesh,
            remesh_resolution=args.remesh_resolution,
        )
    except Exception as e:
        print(f"Erro durante reparo: {e}", file=sys.stderr)
        return 1

    print(f"\nResultado:")
    print(f"  Vértices: {len(cleaned.vertices):,} (antes: {len(mesh.vertices):,})")
    print(f"  Faces: {len(cleaned.faces):,} (antes: {len(mesh.faces):,})")

    # Determina formato de saída
    ext = output_path.suffix.lower()
    if ext in (".glb", ".gltf"):
        file_type = "glb"
    elif ext == ".obj":
        file_type = "obj"
    elif ext == ".ply":
        file_type = "ply"
    elif ext == ".stl":
        file_type = "stl"
    else:
        file_type = "glb"

    print(f"\nSalvando: {output_path}")
    try:
        output_path.parent.mkdir(parents=True, exist_ok=True)
        cleaned.export(str(output_path), file_type=file_type)
    except Exception as e:
        print(f"Erro ao salvar: {e}", file=sys.stderr)
        return 1

    print("Pronto!")
    return 0


if __name__ == "__main__":
    sys.exit(main())
