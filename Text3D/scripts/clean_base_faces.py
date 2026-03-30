#!/usr/bin/env python3
"""
Script para limpar faces pequenas na base do mesh após repair.
Remove faces degeneradas e faz simplificação localizada na base.
"""

import argparse
import sys

import trimesh


def clean_base_faces(mesh_path: str, output_path: str) -> int:
    """Limpa faces pequenas na base do mesh."""
    print(f"Carregando: {mesh_path}")
    mesh = trimesh.load(mesh_path, force="mesh")
    print(f"  Entrada: {len(mesh.vertices):,} verts, {len(mesh.faces):,} faces")

    try:
        import tempfile
        from pathlib import Path

        import pymeshlab

        with tempfile.TemporaryDirectory(prefix="clean_base_") as tmpdir:
            in_ply = str(Path(tmpdir) / "in.ply")
            out_ply = str(Path(tmpdir) / "out.ply")
            mesh.export(in_ply)

            ms = pymeshlab.MeshSet()
            ms.load_new_mesh(in_ply)

            n_faces = ms.current_mesh().face_number()
            print(f"  Faces iniciais: {n_faces:,}")

            # 1. Remover componentes pequenas (< 0.5% do total)
            min_component = max(30, int(n_faces * 0.005))
            ms.meshing_remove_small_components(nbfacemin=min_component)
            print(f"  Após remover componentes pequenas: {ms.current_mesh().face_number():,}")

            # 2. Remover faces nulas (degeneradas)
            ms.meshing_remove_null_faces()
            print(f"  Após remover faces nulas: {ms.current_mesh().face_number():,}")

            # 3. Remover faces duplicadas
            ms.meshing_remove_duplicate_faces()
            ms.meshing_remove_duplicate_vertices()
            ms.meshing_remove_unreferenced_vertices()
            print(f"  Após remover duplicatas: {ms.current_mesh().face_number():,}")

            # 4. Simplificação leve para uniformizar (opcional)
            # Reduzir número de faces em ~10% para remover irregularidades
            target_faces = int(ms.current_mesh().face_number() * 0.9)
            ms.meshing_decimation_quadric_edge_collapse(targetfacenum=target_faces)
            print(f"  Após simplificação: {ms.current_mesh().face_number():,}")

            # 5. Smoothing leve para suavizar base
            ms.apply_coord_taubin_smoothing(stepsmoothnum=3, lambda_=0.3, mu=-0.3)

            ms.save_current_mesh(out_ply)
            result = trimesh.load(out_ply, force="mesh")

            print(f"  Saída: {len(result.vertices):,} verts, {len(result.faces):,} faces")
            result.export(output_path)
            print(f"Salvo: {output_path}")
            return 0

    except Exception as e:
        print(f"Erro: {e}")
        import traceback

        traceback.print_exc()
        return 1


def main() -> int:
    parser = argparse.ArgumentParser(description="Limpa faces pequenas na base do mesh")
    parser.add_argument("input", help="Arquivo GLB/GLTF de entrada")
    parser.add_argument("-o", "--output", required=True, help="Arquivo GLB de saída")
    args = parser.parse_args()
    return clean_base_faces(args.input, args.output)


if __name__ == "__main__":
    sys.exit(main())
