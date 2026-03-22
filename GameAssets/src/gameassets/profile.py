"""Perfil de jogo (game.yaml)."""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import yaml


def _parse_output_dir(raw: Any) -> str:
    """Raiz dos assets (defeito: diretório atual — só `images/` e `meshes/`, sem pasta `outputs/`)."""
    if raw is None:
        return "."
    s = str(raw).strip()
    return s if s else "."


@dataclass
class Text3DProfile:
    """Opções passadas ao CLI text3d generate (subconjunto)."""

    preset: str | None = None  # fast | balanced | hq
    low_vram: bool = False
    texture: bool = False
    # Se qualquer um estiver definido, não se passa --preset (o text3d aplica preset por cima de --steps)
    steps: int | None = None
    octree_resolution: int | None = None
    num_chunks: int | None = None
    no_mesh_repair: bool = False
    mesh_smooth: int | None = None
    mc_level: float | None = None
    # PBR: Text3D --materialize (Materialize CLI → normal, AO, metallic-roughness no GLB)
    materialize: bool = False
    materialize_save_maps: bool = False
    # Se true, o batch copia mapas gerados (staging em tmp) para output_dir/materialize_maps_subdir.
    materialize_export_maps_to_output: bool = False
    materialize_maps_subdir: str = "pbr_maps"
    materialize_bin: str | None = None
    materialize_no_invert: bool = False
    # Repasse ao CLI text3d generate (VRAM / exclusividade GPU)
    allow_shared_gpu: bool = False
    gpu_kill_others: bool = True
    # True: batch em 3 passes — shape (generate) → Paint (texture) → PBR (materialize-pbr)
    # Liberta VRAM entre Hunyuan e Paint; só um tipo de pipeline pesado por subprocesso.
    phased_batch: bool = False
    # GPU pura: desativa CPU offload onde possível (Text2D, Paint).
    # NOTA: O volume decoding do Hunyuan3D shape pode continuar com CPU por limitações da biblioteca hy3dgen.
    full_gpu: bool = False
    # Subpasta do modelo Hunyuan3D shape (ex.: hunyuan3d-dit-v2-mini-turbo para modo turbo)
    model_subfolder: str | None = None


@dataclass
class Text2DProfile:
    """Opções passadas ao CLI text2d generate (subconjunto)."""

    low_vram: bool = False
    cpu: bool = False
    width: int | None = None
    height: int | None = None


@dataclass
class GameProfile:
    title: str
    genre: str
    tone: str
    style_preset: str
    negative_keywords: list[str] = field(default_factory=list)
    output_dir: str = "."
    images_subdir: str = "images"
    meshes_subdir: str = "meshes"
    # split: images_subdir/id.png e meshes_subdir/id.glb (comportamento clássico)
    # flat: output_dir / dirname(id) / basename.ext — PNG e GLB na mesma pasta (ex.: Godot por categoria)
    path_layout: str = "split"
    seed_base: int | None = None
    image_ext: str = "png"
    text2d: Text2DProfile | None = None
    text3d: Text3DProfile | None = None

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> GameProfile:
        if not isinstance(data, dict):
            raise ValueError("Perfil deve ser um mapa YAML")
        required = ("title", "genre", "tone", "style_preset")
        for key in required:
            if key not in data or data[key] in (None, ""):
                raise ValueError(f"Campo obrigatório em game.yaml: {key}")
        neg = data.get("negative_keywords") or []
        if isinstance(neg, str):
            neg = [neg]
        if not isinstance(neg, list):
            raise ValueError("negative_keywords deve ser lista ou string")
        ext = str(data.get("image_ext") or "png").lower().lstrip(".")
        if ext not in ("png", "jpg", "jpeg"):
            raise ValueError("image_ext deve ser png, jpg ou jpeg")
        t2: Text2DProfile | None = None
        raw_t2 = data.get("text2d")
        if isinstance(raw_t2, dict):
            w = raw_t2.get("width")
            h = raw_t2.get("height")
            try:
                wi = int(w) if w is not None else None
                he = int(h) if h is not None else None
            except (TypeError, ValueError) as e:
                raise ValueError("text2d.width e text2d.height devem ser inteiros") from e
            t2 = Text2DProfile(
                low_vram=bool(raw_t2.get("low_vram", False)),
                cpu=bool(raw_t2.get("cpu", False)),
                width=wi,
                height=he,
            )
        t3: Text3DProfile | None = None
        raw_t3 = data.get("text3d")
        if isinstance(raw_t3, dict):
            pr = raw_t3.get("preset")
            if pr is not None and pr not in ("fast", "balanced", "hq"):
                raise ValueError("text3d.preset deve ser fast, balanced ou hq")
            tx = raw_t3.get("texture")
            if tx is None:
                tx = True
            else:
                tx = bool(tx)
            st = raw_t3.get("steps")
            oc = raw_t3.get("octree_resolution")
            nc = raw_t3.get("num_chunks")
            ms = raw_t3.get("mesh_smooth")
            mcl = raw_t3.get("mc_level")
            try:
                st_i = int(st) if st is not None else None
                oc_i = int(oc) if oc is not None else None
                nc_i = int(nc) if nc is not None else None
                ms_i = int(ms) if ms is not None else None
                mcl_f = float(mcl) if mcl is not None else None
            except (TypeError, ValueError) as e:
                raise ValueError(
                    "text3d.steps, octree_resolution, num_chunks, mesh_smooth e mc_level "
                    "devem ser números válidos"
                ) from e
            mat = bool(raw_t3.get("materialize", False))
            if mat:
                tx = True
            mat_save = bool(raw_t3.get("materialize_save_maps", False))
            mat_export = bool(raw_t3.get("materialize_export_maps_to_output", False))
            msd = raw_t3.get("materialize_maps_subdir")
            msd_s = str(msd).strip() if msd not in (None, "") else "pbr_maps"
            mat_bin = raw_t3.get("materialize_bin")
            mat_bin_s = str(mat_bin).strip() if mat_bin not in (None, "") else None
            mat_noinv = bool(raw_t3.get("materialize_no_invert", False))
            allow_sg = bool(raw_t3.get("allow_shared_gpu", False))
            gko = raw_t3.get("gpu_kill_others")
            g_kill = True if gko is None else bool(gko)
            phased = bool(raw_t3.get("phased_batch", False))
            full_gpu = bool(raw_t3.get("full_gpu", False))
            model_sub = raw_t3.get("model_subfolder")
            model_sub_s = str(model_sub).strip() if model_sub not in (None, "") else None
            t3 = Text3DProfile(
                preset=pr,
                low_vram=bool(raw_t3.get("low_vram", False)),
                texture=tx,
                steps=st_i,
                octree_resolution=oc_i,
                num_chunks=nc_i,
                no_mesh_repair=bool(raw_t3.get("no_mesh_repair", False)),
                mesh_smooth=ms_i,
                mc_level=mcl_f,
                materialize=mat,
                materialize_save_maps=mat_save,
                materialize_export_maps_to_output=mat_export,
                materialize_maps_subdir=msd_s,
                materialize_bin=mat_bin_s,
                materialize_no_invert=mat_noinv,
                allow_shared_gpu=allow_sg,
                gpu_kill_others=g_kill,
                phased_batch=phased,
                full_gpu=full_gpu,
                model_subfolder=model_sub_s,
            )
        sb = data.get("seed_base")
        if sb is not None:
            try:
                sb = int(sb)
            except (TypeError, ValueError) as e:
                raise ValueError("seed_base deve ser um inteiro") from e
        pl = str(data.get("path_layout") or "split").strip().lower()
        if pl not in ("split", "flat"):
            raise ValueError("path_layout deve ser split ou flat")
        return cls(
            title=str(data["title"]),
            genre=str(data["genre"]),
            tone=str(data["tone"]),
            style_preset=str(data["style_preset"]),
            negative_keywords=[str(x) for x in neg],
            output_dir=_parse_output_dir(data.get("output_dir")),
            images_subdir=str(data.get("images_subdir") or "images"),
            meshes_subdir=str(data.get("meshes_subdir") or "meshes"),
            path_layout=pl,
            seed_base=sb,
            image_ext="jpg" if ext == "jpeg" else ext,
            text2d=t2,
            text3d=t3,
        )


def load_profile(path: Path) -> GameProfile:
    with path.open("r", encoding="utf-8") as f:
        raw = yaml.safe_load(f)
    return GameProfile.from_dict(raw or {})
