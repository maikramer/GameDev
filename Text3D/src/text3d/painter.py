"""
Textura com Hunyuan3D-Paint (hy3dgen.texgen.Hunyuan3DPaintPipeline).

Requer pesos em ``tencent/Hunyuan3D-2`` (subpastas delight + paint), descarregados na primeira execução.
"""

from __future__ import annotations

from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path

import torch
import trimesh
from PIL import Image

from . import defaults as _defaults
from .utils.memory import clear_cuda_memory

_PAINT_RASTERIZER_HINT = (
    "O Hunyuan3D-Paint precisa do módulo CUDA `custom_rasterizer` (extensão compilada). "
    "Instala o CUDA Toolkit (nvcc), define CUDA_HOME, depois:\n"
    "  git clone --depth 1 https://github.com/Tencent-Hunyuan/Hunyuan3D-2.git\n"
    "  cd Hunyuan3D-2/hy3dgen/texgen/custom_rasterizer\n"
    "  pip install -e . --no-build-isolation\n"
    "Ver também: docs/PAINT_SETUP.md no repositório Text3D."
)

# Steps originais da lib: delight=50, multiview=30.
# Turbo (LCMScheduler) converge muito mais rápido.
PAINT_DELIGHT_STEPS = 20
PAINT_MULTIVIEW_STEPS_TURBO = 8
PAINT_RENDER_SIZE = 1024
PAINT_TEXTURE_SIZE = 1024


@contextmanager
def _paint_config_optimized(render_size: int, texture_size: int) -> Iterator[None]:
    """
    Força device='cpu' (evita OOM no carregamento) e aplica render/texture size
    reduzidos para acelerar baking e reduzir VRAM.
    """
    import hy3dgen.texgen.pipelines as pip_mod

    orig = pip_mod.Hunyuan3DTexGenConfig.__init__

    def wrapped_init(self, *args, **kwargs):
        orig(self, *args, **kwargs)
        self.device = "cpu"
        self.render_size = render_size
        self.texture_size = texture_size

    pip_mod.Hunyuan3DTexGenConfig.__init__ = wrapped_init  # type: ignore[assignment]
    try:
        yield
    finally:
        pip_mod.Hunyuan3DTexGenConfig.__init__ = orig  # type: ignore[assignment]


def _patch_delight_steps(pipe, steps: int) -> None:
    """Reduz os steps do Delight (SD InstructPix2Pix) de 50 para ``steps``."""
    delight = pipe.models.get("delight_model")
    if delight is None:
        return

    def patched_call(self, image):
        image = image.resize((512, 512))

        import cv2
        import numpy as np

        if image.mode == "RGBA":
            image_array = np.array(image)
            alpha_channel = image_array[:, :, 3]
            kernel = np.ones((3, 3), np.uint8)
            alpha_channel = cv2.erode(alpha_channel, kernel, iterations=1)
            image_array[alpha_channel == 0, :3] = 255
            image_array[:, :, 3] = alpha_channel
            image = Image.fromarray(image_array)
            image_tensor = torch.tensor(np.array(image) / 255.0).to(self.device)
            alpha = image_tensor[:, :, 3:]
            rgb_target = image_tensor[:, :, :3]
        else:
            image_tensor = torch.tensor(np.array(image) / 255.0).to(self.device)
            alpha = torch.ones_like(image_tensor)[:, :, :1]
            rgb_target = image_tensor[:, :, :3]

        image = image.convert("RGB")
        image = self.pipeline(
            prompt="",
            image=image,
            generator=torch.manual_seed(42),
            height=512,
            width=512,
            num_inference_steps=steps,
            image_guidance_scale=self.cfg_image,
            guidance_scale=self.cfg_text,
        ).images[0]

        image_tensor = torch.tensor(np.array(image) / 255.0).to(self.device)
        rgb_src = image_tensor[:, :, :3]
        image = self.recorrect_rgb(rgb_src, rgb_target, alpha)
        image = image[:, :, :3] * image[:, :, 3:] + torch.ones_like(image[:, :, :3]) * (1.0 - image[:, :, 3:])
        image = Image.fromarray((image.cpu().numpy() * 255).astype(np.uint8))
        return image

    import types

    delight.__call__ = types.MethodType(patched_call, delight)


def _patch_multiview_steps(pipe, steps: int) -> None:
    """Reduz os steps do Multiview diffusion de 30 para ``steps``."""
    mv = pipe.models.get("multiview_model")
    if mv is None:
        return

    def patched_call(self, input_images, control_images, camera_info):
        self.seed_everything(0)
        if not isinstance(input_images, list):
            input_images = [input_images]
        input_images = [img.resize((self.view_size, self.view_size)) for img in input_images]
        for i in range(len(control_images)):
            control_images[i] = control_images[i].resize((self.view_size, self.view_size))
            if control_images[i].mode == "L":
                control_images[i] = control_images[i].point(lambda x: 255 if x > 1 else 0, mode="1")

        kwargs = dict(generator=torch.Generator(device=self.pipeline.device).manual_seed(0))
        num_view = len(control_images) // 2
        kwargs["width"] = self.view_size
        kwargs["height"] = self.view_size
        kwargs["num_in_batch"] = num_view
        kwargs["camera_info_gen"] = [camera_info]
        kwargs["camera_info_ref"] = [[0]]
        kwargs["normal_imgs"] = [[control_images[i] for i in range(num_view)]]
        kwargs["position_imgs"] = [[control_images[i + num_view] for i in range(num_view)]]

        return self.pipeline(input_images, num_inference_steps=steps, **kwargs).images

    import types

    mv.__call__ = types.MethodType(patched_call, mv)


def check_paint_rasterizer_available() -> None:
    """Falha cedo com mensagem clara se o rasterizador CUDA do texgen não estiver instalado."""
    try:
        import custom_rasterizer  # noqa: F401
        import torch  # noqa: F401
    except (ImportError, ModuleNotFoundError, OSError) as e:
        raise RuntimeError(_PAINT_RASTERIZER_HINT) from e


def load_mesh_trimesh(path: str | Path) -> trimesh.Trimesh:
    """Carrega GLB/OBJ/PLY e devolve um único Trimesh (fundir cenas)."""
    path = Path(path)
    loaded = trimesh.load(str(path), force=None)
    if isinstance(loaded, trimesh.Scene):
        if not loaded.geometry:
            raise ValueError(f"Mesh vazia: {path}")
        meshes = list(loaded.geometry.values())
        if len(meshes) == 1:
            return meshes[0]
        return trimesh.util.concatenate(meshes)
    if isinstance(loaded, trimesh.Trimesh):
        return loaded
    raise TypeError(f"Formato não suportado: {type(loaded)}")


def apply_hunyuan_paint(
    mesh: trimesh.Trimesh,
    image: str | Path | Image.Image,
    *,
    model_repo: str = _defaults.DEFAULT_PAINT_HF_REPO,
    subfolder: str = _defaults.DEFAULT_PAINT_SUBFOLDER,
    paint_cpu_offload: bool = _defaults.DEFAULT_PAINT_CPU_OFFLOAD,
    verbose: bool = False,
) -> trimesh.Trimesh:
    """
    Aplica Hunyuan3D-Paint: mesh + imagem de referência → mesh com UV e textura embutida (GLB).

    ``image`` deve alinhar semanticamente com a geometria (ex.: a mesma imagem usada no image-to-3D).
    """
    check_paint_rasterizer_available()

    from hy3dgen.texgen import Hunyuan3DPaintPipeline

    if verbose:
        print(f"[Paint] repo={model_repo} subfolder={subfolder} offload={paint_cpu_offload}")
        print(
            f"[Paint] delight_steps={PAINT_DELIGHT_STEPS} "
            f"multiview_steps={PAINT_MULTIVIEW_STEPS_TURBO} "
            f"render={PAINT_RENDER_SIZE} texture={PAINT_TEXTURE_SIZE}"
        )

    clear_cuda_memory()

    with _paint_config_optimized(PAINT_RENDER_SIZE, PAINT_TEXTURE_SIZE):
        pipe = Hunyuan3DPaintPipeline.from_pretrained(model_repo, subfolder=subfolder)

    _patch_delight_steps(pipe, PAINT_DELIGHT_STEPS)
    _patch_multiview_steps(pipe, PAINT_MULTIVIEW_STEPS_TURBO)

    if torch.cuda.is_available() and paint_cpu_offload:
        pipe.enable_model_cpu_offload()

    if isinstance(image, (str, Path)):
        img_arg: str | Image.Image = str(image)
    else:
        img_arg = image.convert("RGB") if image.mode != "RGB" else image

    try:
        with torch.inference_mode():
            textured = pipe(mesh, img_arg)
    finally:
        del pipe
        clear_cuda_memory()

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
    verbose: bool = False,
    materialize: bool = False,
    materialize_output_dir: str | Path | None = None,
    materialize_bin: str | Path | None = None,
    materialize_no_invert: bool = False,
) -> Path:
    """Atalho: carrega mesh, pinta, exporta GLB. Com ``materialize=True``, embute PBR (Materialize CLI)."""
    repo = model_repo or _defaults.DEFAULT_PAINT_HF_REPO
    sub = subfolder or _defaults.DEFAULT_PAINT_SUBFOLDER
    offload = _defaults.DEFAULT_PAINT_CPU_OFFLOAD if paint_cpu_offload is None else paint_cpu_offload

    mesh = load_mesh_trimesh(mesh_path)
    out = apply_hunyuan_paint(
        mesh,
        image_path,
        model_repo=repo,
        subfolder=sub,
        paint_cpu_offload=offload,
        verbose=verbose,
    )
    if materialize:
        from .materialize_pbr import apply_materialize_pbr

        out = apply_materialize_pbr(
            out,
            materialize_bin=materialize_bin,
            save_sidecar_maps_dir=materialize_output_dir,
            roughness_from_one_minus_smoothness=not materialize_no_invert,
            verbose=verbose,
        )
    output_path = Path(output_path)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    out.export(str(output_path), file_type="glb")
    return output_path
