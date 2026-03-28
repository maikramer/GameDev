"""
Utilitários para exportação e conversão de arquivos 3D.
"""

import warnings
from pathlib import Path

import numpy as np
import trimesh
from diffusers.utils import export_to_gif, export_to_obj, export_to_ply

from ..defaults import get_export_rotation_x_rad


def _export_glb_with_normals(mesh: trimesh.Trimesh, output_path: Path) -> None:
    """Export GLB ensuring vertex normals and doubleSided are included."""
    _ = mesh.vertex_normals  # force computation
    if hasattr(mesh, "visual") and hasattr(mesh.visual, "material"):
        mesh.visual.material.doubleSided = True
    scene = trimesh.Scene(geometry={"mesh": mesh})
    glb_bytes = scene.export(file_type="glb", include_normals=True)
    with open(str(output_path), "wb") as f:
        f.write(glb_bytes)


def save_mesh(
    mesh_input: np.ndarray | trimesh.Trimesh,
    output_path: str | Path,
    format: str | None = None,
    rotate: bool = True,
) -> Path:
    """
    Salva mesh em formato PLY, OBJ ou GLB.

    Aceita array numpy (legado / diffusers) ou ``trimesh.Trimesh`` (Hunyuan3D).
    """
    output_path = Path(output_path)

    if format is None:
        format = output_path.suffix.lstrip(".")

    format = format.lower()
    output_path.parent.mkdir(parents=True, exist_ok=True)

    if isinstance(mesh_input, trimesh.Trimesh):
        mesh = mesh_input
        if rotate:
            mesh = _apply_rotation_trimesh(mesh.copy())
        if format == "glb":
            _export_glb_with_normals(mesh, output_path)
            return output_path
        if format == "ply":
            mesh.export(str(output_path), file_type="ply")
            return output_path
        if format == "obj":
            mesh.export(str(output_path), file_type="obj")
            return output_path
        raise ValueError(f"Formato não suportado: {format}")

    mesh_array = mesh_input

    if format == "ply":
        temp_ply = output_path.with_suffix(".ply")
        export_to_ply(mesh_array, str(temp_ply))

        if rotate:
            _apply_rotation(temp_ply)

        if output_path.suffix.lower() == ".ply":
            return temp_ply

        if format != "ply":
            return convert_mesh(temp_ply, output_path, rotate=False)

    elif format == "obj":
        export_to_obj(mesh_array, str(output_path))

        if rotate:
            _apply_rotation(output_path)

    elif format == "glb":
        temp_ply = output_path.with_suffix(".temp.ply")
        export_to_ply(mesh_array, str(temp_ply))

        try:
            mesh = trimesh.load(temp_ply)

            if rotate:
                mesh = _apply_rotation_trimesh(mesh)

            mesh.export(str(output_path), file_type="glb")
        finally:
            if temp_ply.exists():
                temp_ply.unlink()

    else:
        raise ValueError(f"Formato não suportado: {format}")

    return output_path


def save_gif(
    frames: list,
    output_path: str | Path,
) -> Path:
    """
    Salva frames como GIF animado.

    Args:
        frames: Lista de frames PIL.Image
        output_path: Caminho de saída

    Returns:
        Caminho do arquivo salvo
    """
    output_path = Path(output_path)
    output_path.parent.mkdir(parents=True, exist_ok=True)

    export_to_gif(frames, str(output_path))

    return output_path


def convert_mesh(
    input_path: str | Path,
    output_path: str | Path,
    rotate: bool = False,
) -> Path:
    """
    Converte mesh entre formatos usando trimesh.

    Args:
        input_path: Arquivo de entrada
        output_path: Arquivo de saída
        rotate: Aplicar rotação

    Returns:
        Caminho do arquivo convertido
    """
    input_path = Path(input_path)
    output_path = Path(output_path)

    if not input_path.exists():
        raise FileNotFoundError(f"Arquivo não encontrado: {input_path}")

    # Carregar mesh
    mesh = trimesh.load(input_path)

    if rotate:
        mesh = _apply_rotation_trimesh(mesh)

    # Determinar formato de saída
    output_format = output_path.suffix.lstrip(".").lower()

    # Exportar
    mesh.export(str(output_path), file_type=output_format)

    return output_path


def _apply_rotation(ply_path: Path):
    """
    Aplica rotação para orientar mesh corretamente (eixo Y para cima).

    Args:
        ply_path: Caminho do arquivo PLY
    """
    try:
        mesh = trimesh.load(ply_path)
        mesh = _apply_rotation_trimesh(mesh)
        mesh.export(str(ply_path), file_type="ply")
    except Exception as e:
        warnings.warn(f"Não foi possível aplicar rotação: {e}", stacklevel=2)


def _apply_rotation_trimesh(mesh: trimesh.Trimesh) -> trimesh.Trimesh:
    """
    Rotação X Hunyuan3D → Y-up (Godot/Blender). Ângulo: ``get_export_rotation_x_rad()`` (+90° por defeito).
    """
    angle = float(get_export_rotation_x_rad())
    axis = [1, 0, 0]
    rotation_matrix = trimesh.transformations.rotation_matrix(angle, axis)
    mesh.apply_transform(rotation_matrix)
    return mesh


def get_mesh_info(mesh_path: str | Path) -> dict:
    """
    Obtém informações sobre um arquivo mesh.

    Args:
        mesh_path: Caminho do arquivo mesh

    Returns:
        Dicionário com informações
    """
    mesh_path = Path(mesh_path)

    if not mesh_path.exists():
        raise FileNotFoundError(f"Arquivo não encontrado: {mesh_path}")

    mesh = trimesh.load(mesh_path)

    info = {
        "path": str(mesh_path),
        "format": mesh_path.suffix.lstrip(".").lower(),
        "vertices": len(mesh.vertices),
        "faces": len(mesh.faces) if hasattr(mesh, "faces") else 0,
        "bounds": mesh.bounds.tolist(),
        "is_watertight": mesh.is_watertight if hasattr(mesh, "is_watertight") else None,
        "volume": mesh.volume if hasattr(mesh, "volume") and mesh.is_watertight else None,
    }

    return info
