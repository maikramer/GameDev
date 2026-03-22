"""
PBR extra (normal, oclusão, metallic-roughness) via Materialize CLI + glTF 2.0 no Trimesh.

Requer o binário ``materialize`` no PATH (ou ``MATERIALIZE_BIN`` / argumento explícito).
"""

from __future__ import annotations

import os
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Optional, Tuple, Union

import numpy as np
import trimesh
from PIL import Image

def resolve_materialize_binary(explicit: Optional[Union[str, Path]] = None) -> str:
    """Caminho para o executável Materialize CLI."""
    if explicit is not None:
        p = Path(explicit)
        if not p.is_file():
            raise FileNotFoundError(f"Materialize não encontrado: {explicit}")
        return str(p.resolve())
    env = os.environ.get("MATERIALIZE_BIN", "").strip()
    if env:
        pe = Path(env)
        if not pe.is_file():
            raise FileNotFoundError(f"MATERIALIZE_BIN inválido: {env}")
        return str(pe.resolve())
    found = shutil.which("materialize")
    if not found:
        raise RuntimeError(
            "Materialize CLI não está no PATH. Instala o binário (ex.: GameDev/Materialize) "
            "ou define MATERIALIZE_BIN=/caminho/para/materialize."
        )
    return found


def extract_base_color_and_uv(mesh: trimesh.Trimesh) -> Tuple[np.ndarray, Image.Image]:
    """
    Extrai UVs e imagem base (albedo) de uma mesh com ``TextureVisuals``.
    Aceita ``SimpleMaterial`` ou ``PBRMaterial`` (usa ``baseColorTexture``).
    """
    vis = mesh.visual
    if vis is None:
        raise ValueError("Mesh sem componente visual")
    kind = getattr(vis, "kind", None)
    if kind != "texture":
        raise ValueError(f"Esperado visual texturado (kind=texture), obtido {kind!r}")

    uv = np.asarray(vis.uv, dtype=np.float64)
    mat = vis.material

    img: Optional[Image.Image] = None
    if isinstance(mat, trimesh.visual.material.PBRMaterial):
        img = mat.baseColorTexture
    elif isinstance(mat, trimesh.visual.material.SimpleMaterial):
        img = mat.image
    elif hasattr(mat, "to_pbr"):
        pbr = mat.to_pbr()
        if isinstance(pbr, trimesh.visual.material.PBRMaterial):
            img = pbr.baseColorTexture

    if img is None:
        raise ValueError("Não foi possível obter textura base (albedo) do material")

    if img.mode == "RGBA":
        rgb = Image.new("RGB", img.size, (255, 255, 255))
        rgb.paste(img, mask=img.split()[3])
        img = rgb
    else:
        img = img.convert("RGB")

    return uv, img


def _luma01(img: Image.Image) -> np.ndarray:
    """Escala 0–1 a partir de L ou RGB (média dos canais)."""
    g = np.asarray(img.convert("L"), dtype=np.float64) / 255.0
    return g


def pack_metallic_roughness_gltf(
    metallic: Image.Image,
    smoothness: Image.Image,
    *,
    roughness_from_one_minus_smoothness: bool = True,
) -> Image.Image:
    """
    Empacota texturas glTF ``metallicRoughness``: canal G = roughness, B = metallic.
    Por defeito: roughness = 1 - smoothness (workflow estilo Unity smoothness).
    """
    if metallic.size != smoothness.size:
        smoothness = smoothness.resize(metallic.size, Image.Resampling.BILINEAR)
    m = _luma01(metallic)
    s = _luma01(smoothness)
    if roughness_from_one_minus_smoothness:
        r = 1.0 - s
    else:
        r = s
    r = np.clip(r, 0.0, 1.0)
    m = np.clip(m, 0.0, 1.0)
    h, w = m.shape
    rgba = np.zeros((h, w, 4), dtype=np.uint8)
    rgba[:, :, 0] = 255
    rgba[:, :, 1] = (r * 255.0).astype(np.uint8)
    rgba[:, :, 2] = (m * 255.0).astype(np.uint8)
    rgba[:, :, 3] = 255
    return Image.fromarray(rgba, mode="RGBA")


def _expected_materialize_paths(albedo_stem: str, out_dir: Path, ext: str = "png") -> dict:
    ext = ext.lower()
    if ext == "jpeg":
        ext = "jpg"
    suf = "jpg" if ext == "jpg" else ext
    return {
        "normal": out_dir / f"{albedo_stem}_normal.{suf}",
        "metallic": out_dir / f"{albedo_stem}_metallic.{suf}",
        "smoothness": out_dir / f"{albedo_stem}_smoothness.{suf}",
        "ao": out_dir / f"{albedo_stem}_ao.{suf}",
    }


def run_materialize_cli(
    albedo_path: Path,
    output_dir: Path,
    *,
    materialize_bin: Optional[Union[str, Path]] = None,
    image_format: str = "png",
    verbose: bool = False,
) -> dict:
    """
    Executa ``materialize`` e devolve caminhos para normal, metallic, smoothness, ao.
    """
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    bin_path = resolve_materialize_binary(materialize_bin)
    cmd = [
        bin_path,
        str(albedo_path),
        "-o",
        str(output_dir),
        "-f",
        image_format,
    ]
    if verbose:
        cmd.append("-v")
    r = subprocess.run(cmd, capture_output=True, text=True)
    if r.returncode != 0:
        err = (r.stderr or r.stdout or "").strip()
        raise RuntimeError(f"materialize falhou (exit {r.returncode}): {err}")

    stem = albedo_path.stem
    paths = _expected_materialize_paths(stem, output_dir, image_format)
    for key, p in paths.items():
        if not p.is_file():
            raise FileNotFoundError(f"Mapa em falta após materialize: {p}")
    return paths


def apply_materialize_pbr(
    mesh: trimesh.Trimesh,
    *,
    materialize_bin: Optional[Union[str, Path]] = None,
    work_dir: Optional[Union[str, Path]] = None,
    save_sidecar_maps_dir: Optional[Union[str, Path]] = None,
    roughness_from_one_minus_smoothness: bool = True,
    verbose: bool = False,
) -> trimesh.Trimesh:
    """
    Gera mapas PBR com Materialize a partir do albedo embutido e substitui o material por ``PBRMaterial``
    (baseColor + normal + occlusion + metallicRoughness), preservando UVs.
    """
    uv, base_color = extract_base_color_and_uv(mesh)

    cleanup = work_dir is None
    wd = Path(work_dir) if work_dir is not None else Path(tempfile.mkdtemp(prefix="text3d_materialize_"))
    try:
        wd.mkdir(parents=True, exist_ok=True)
        albedo_path = wd / "text3d_albedo.png"
        base_color.save(albedo_path, format="PNG")

        paths = run_materialize_cli(
            albedo_path,
            wd,
            materialize_bin=materialize_bin,
            image_format="png",
            verbose=verbose,
        )

        normal = Image.open(paths["normal"]).convert("RGB")
        metallic = Image.open(paths["metallic"])
        smoothness = Image.open(paths["smoothness"])
        ao = Image.open(paths["ao"]).convert("L")

        mr = pack_metallic_roughness_gltf(
            metallic,
            smoothness,
            roughness_from_one_minus_smoothness=roughness_from_one_minus_smoothness,
        )

        if save_sidecar_maps_dir is not None:
            side = Path(save_sidecar_maps_dir)
            side.mkdir(parents=True, exist_ok=True)
            for src in paths.values():
                shutil.copy2(src, side / Path(src).name)
            base_color.save(side / "baseColor.png", format="PNG")
            mr.save(side / "metallicRoughness.png", format="PNG")
            ao.save(side / "occlusion.png", format="PNG")

        pbr = trimesh.visual.material.PBRMaterial(
            baseColorTexture=base_color,
            normalTexture=normal,
            occlusionTexture=ao.convert("RGB"),
            metallicRoughnessTexture=mr,
            metallicFactor=1.0,
            roughnessFactor=1.0,
        )
        mesh = mesh.copy()
        mesh.visual = trimesh.visual.TextureVisuals(uv=uv, material=pbr)
        return mesh
    finally:
        if cleanup and wd.exists():
            shutil.rmtree(wd, ignore_errors=True)
