"""
Textura com Hunyuan3D-Paint 2.1 (``hy3dpaint.textureGenPipeline.Hunyuan3DPaintPipeline``).

Requer o código em ``third_party/Hunyuan3D-2.1/hy3dpaint`` (submodule) ou ``HUNYUAN3D_21_ROOT``,
pesos em Hugging Face (``tencent/Hunyuan3D-2.1``, pasta ``hunyuan3d-paintpbr-v2-1``),
e o checkpoint Real-ESRGAN em ``hy3dpaint/ckpt/RealESRGAN_x4plus.pth``.

O rasterizador CUDA é fornecido por **nvdiffrast** (NVIDIA), registado como
``custom_rasterizer`` em ``sys.modules`` antes de importar o renderer 2.1.
"""

from __future__ import annotations

import shutil
import sys
import tempfile
from pathlib import Path

import torch
import trimesh
from PIL import Image

from gamedev_shared.gpu import clear_cuda_memory

from . import defaults as _defaults
from .hy3d21_paths import default_realesrgan_ckpt, ensure_hy3dpaint_on_path, resolve_hy3dpaint_root
from .utils.mesh_io import load_mesh_trimesh, save_glb


def _ensure_custom_rasterizer_shim() -> None:
    """Regista o shim nvdiffrast como ``custom_rasterizer`` se o módulo nativo não existir."""
    if "custom_rasterizer" in sys.modules:
        return
    try:
        import custom_rasterizer  # noqa: F401 - extensão nativa já instalada

        return
    except (ImportError, ModuleNotFoundError, OSError):
        pass

    from paint3d import custom_rasterizer_shim

    sys.modules["custom_rasterizer"] = custom_rasterizer_shim  # type: ignore[assignment]


def check_paint_rasterizer_available() -> None:
    """Garante que ``custom_rasterizer`` está disponível (shim nvdiffrast ou extensão nativa)."""
    _ensure_custom_rasterizer_shim()
    try:
        import custom_rasterizer  # noqa: F401
    except (ImportError, ModuleNotFoundError, OSError) as e:
        raise RuntimeError(
            "Rasterizador indisponível: nem nvdiffrast nem custom_rasterizer foram encontrados.\n"
            "Instala nvdiffrast: pip install git+https://github.com/NVlabs/nvdiffrast.git --no-build-isolation\n"
            "Ou compila hy3dpaint/custom_rasterizer (ver PAINT_SETUP.md)."
        ) from e


def check_hunyuan3d21_environment() -> tuple[bool, str]:
    """
    Verifica clone hy3dpaint e peso Real-ESRGAN.
    Devolve (ok, mensagem ou caminho do hy3dpaint).
    """
    try:
        root = resolve_hy3dpaint_root()
    except FileNotFoundError as e:
        return False, str(e)
    ckpt = default_realesrgan_ckpt(root)
    if not ckpt.is_file():
        return False, f"Real-ESRGAN em falta: {ckpt} (ver PAINT_SETUP.md)"
    cfg = root / "cfgs" / "hunyuan-paint-pbr.yaml"
    if not cfg.is_file():
        return False, f"Config em falta: {cfg}"
    return True, str(root)


def apply_hunyuan_paint(
    mesh: trimesh.Trimesh,
    image: str | Path | Image.Image,
    *,
    model_repo: str = _defaults.DEFAULT_PAINT_HF_REPO,
    subfolder: str = _defaults.DEFAULT_PAINT_SUBFOLDER,
    paint_cpu_offload: bool = _defaults.DEFAULT_PAINT_CPU_OFFLOAD,
    max_num_view: int = _defaults.DEFAULT_PAINT_MAX_VIEWS,
    view_resolution: int = _defaults.DEFAULT_PAINT_VIEW_RESOLUTION,
    use_remesh: bool = True,
    verbose: bool = False,
) -> trimesh.Trimesh:
    """
    Aplica Hunyuan3D-Paint 2.1: mesh + imagem de referência → mesh com UV e textura/PBR (GLB).

    ``image`` deve alinhar semanticamente com a geometria (ex.: a mesma imagem usada no image-to-3D).
    """
    check_paint_rasterizer_available()

    ok, msg = check_hunyuan3d21_environment()
    if not ok:
        raise RuntimeError(msg)

    hy3dpaint_root = ensure_hy3dpaint_on_path()
    cfg_yaml = hy3dpaint_root / "cfgs" / "hunyuan-paint-pbr.yaml"
    ckpt_path = default_realesrgan_ckpt(hy3dpaint_root)

    from textureGenPipeline import Hunyuan3DPaintConfig, Hunyuan3DPaintPipeline

    if verbose:
        print(
            f"[Paint 2.1] hy3dpaint={hy3dpaint_root}\n"
            f"  repo={model_repo} weights_subfolder={subfolder} offload={paint_cpu_offload} "
            f"max_views={max_num_view} res={view_resolution}"
        )

    clear_cuda_memory()

    with tempfile.TemporaryDirectory(prefix="paint3d_h21_") as td_raw:
        tdir = Path(td_raw)
        mesh_in = tdir / "input_mesh.glb"
        ref_path = tdir / "ref.png"
        out_obj = tdir / "textured_mesh.obj"
        out_glb = tdir / "textured_mesh.glb"

        save_glb(mesh, mesh_in)

        if isinstance(image, (str, Path)):
            shutil.copy2(image, ref_path)
        else:
            im = image.convert("RGB") if image.mode != "RGB" else image
            im.save(ref_path)

        config = Hunyuan3DPaintConfig(max_num_view, view_resolution)
        config.multiview_pretrained_path = model_repo
        config.multiview_weights_subfolder = subfolder
        config.multiview_cfg_path = str(cfg_yaml)
        config.realesrgan_ckpt_path = str(ckpt_path)

        if torch.cuda.is_available():
            config.device = "cuda"
        else:
            config.device = "cpu"

        if paint_cpu_offload and torch.cuda.is_available():
            config.render_size = 1024
            config.texture_size = 2048
        elif not paint_cpu_offload and torch.cuda.is_available():
            pass
        else:
            config.render_size = min(config.render_size, 1024)
            config.texture_size = min(config.texture_size, 2048)

        pipe = Hunyuan3DPaintPipeline(config)

        if paint_cpu_offload and torch.cuda.is_available():
            mv = pipe.models.get("multiview_model")
            pl = getattr(mv, "pipeline", None) if mv is not None else None
            if pl is not None and hasattr(pl, "enable_model_cpu_offload"):
                try:
                    pl.enable_model_cpu_offload()
                except Exception:
                    if verbose:
                        print("[Paint 2.1] enable_model_cpu_offload não aplicável; continua.")

        try:
            with torch.inference_mode():
                pipe(
                    mesh_path=str(mesh_in),
                    image_path=str(ref_path),
                    output_mesh_path=str(out_obj),
                    use_remesh=use_remesh,
                    save_glb=True,
                )
        finally:
            del pipe
            clear_cuda_memory()

        if not out_glb.is_file():
            raise FileNotFoundError(f"Paint 2.1 não gerou GLB esperado: {out_glb}")

        textured = load_mesh_trimesh(out_glb)

    if not isinstance(textured, trimesh.Trimesh):
        raise TypeError(f"Paint devolveu {type(textured)}, esperado Trimesh")

    return textured


def paint_file_to_file(
    mesh_path: str | Path,
    image_path: str | Path,
    output_path: str | Path,
    *,
    model_repo: str | None = None,
    subfolder: str | None = None,
    paint_cpu_offload: bool | None = None,
    max_num_view: int | None = None,
    view_resolution: int | None = None,
    use_remesh: bool = True,
    verbose: bool = False,
) -> Path:
    """Atalho: carrega mesh, pinta com Hunyuan3D-Paint 2.1 (PBR baked), exporta GLB."""
    repo = model_repo or _defaults.DEFAULT_PAINT_HF_REPO
    sub = subfolder or _defaults.DEFAULT_PAINT_SUBFOLDER
    offload = _defaults.DEFAULT_PAINT_CPU_OFFLOAD if paint_cpu_offload is None else paint_cpu_offload
    nviews = _defaults.DEFAULT_PAINT_MAX_VIEWS if max_num_view is None else max_num_view
    vres = _defaults.DEFAULT_PAINT_VIEW_RESOLUTION if view_resolution is None else view_resolution

    mesh = load_mesh_trimesh(mesh_path)
    out = apply_hunyuan_paint(
        mesh,
        image_path,
        model_repo=repo,
        subfolder=sub,
        paint_cpu_offload=offload,
        max_num_view=nviews,
        view_resolution=vres,
        use_remesh=use_remesh,
        verbose=verbose,
    )

    output_path = Path(output_path)
    save_glb(out, output_path)
    return output_path
