"""
Textura com Hunyuan3D-Paint (hy3dgen.texgen.Hunyuan3DPaintPipeline).

Requer pesos em ``tencent/Hunyuan3D-2`` (subpastas delight + paint), descarregados na primeira execução.
"""

from __future__ import annotations

from contextlib import contextmanager
from pathlib import Path
from typing import Iterator, Optional, Union

import torch
import trimesh
from PIL import Image

from . import defaults as _defaults

_PAINT_RASTERIZER_HINT = (
    "O Hunyuan3D-Paint precisa do módulo CUDA `custom_rasterizer` (extensão compilada). "
    "Instala o CUDA Toolkit (nvcc), define CUDA_HOME, depois:\n"
    "  git clone --depth 1 https://github.com/Tencent-Hunyuan/Hunyuan3D-2.git\n"
    "  cd Hunyuan3D-2/hy3dgen/texgen/custom_rasterizer\n"
    "  pip install -e . --no-build-isolation\n"
    "Ver também: docs/PAINT_SETUP.md no repositório Text3D."
)


@contextmanager
def _paint_config_load_weights_on_cpu_first() -> Iterator[None]:
    """
    O hy3dgen carrega Delight + Multiview com .to(cuda) antes do offload — OOM em ~6GB.
    Força ``device='cpu'`` na config para o carregamento inicial; depois
    ``enable_model_cpu_offload`` move blocos para a GPU durante a inferência.
    """
    import hy3dgen.texgen.pipelines as pip_mod

    orig = pip_mod.Hunyuan3DTexGenConfig.__init__

    def wrapped_init(self, *args, **kwargs):
        orig(self, *args, **kwargs)
        self.device = "cpu"

    pip_mod.Hunyuan3DTexGenConfig.__init__ = wrapped_init  # type: ignore[assignment]
    try:
        yield
    finally:
        pip_mod.Hunyuan3DTexGenConfig.__init__ = orig  # type: ignore[assignment]


def check_paint_rasterizer_available() -> None:
    """Falha cedo com mensagem clara se o rasterizador CUDA do texgen não estiver instalado."""
    try:
        import torch  # noqa: F401 — libc10 do kernel liga ao PyTorch
        import custom_rasterizer  # noqa: F401
    except (ImportError, ModuleNotFoundError, OSError) as e:
        raise RuntimeError(_PAINT_RASTERIZER_HINT) from e


def load_mesh_trimesh(path: Union[str, Path]) -> trimesh.Trimesh:
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
    image: Union[str, Path, Image.Image],
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

    if torch.cuda.is_available():
        torch.cuda.empty_cache()

    with _paint_config_load_weights_on_cpu_first():
        pipe = Hunyuan3DPaintPipeline.from_pretrained(model_repo, subfolder=subfolder)

    if torch.cuda.is_available() and paint_cpu_offload:
        pipe.enable_model_cpu_offload()

    if isinstance(image, (str, Path)):
        img_arg: Union[str, Image.Image] = str(image)
    else:
        img_arg = image.convert("RGB") if image.mode != "RGB" else image

    with torch.no_grad():
        textured = pipe(mesh, img_arg)

    if not isinstance(textured, trimesh.Trimesh):
        raise TypeError(f"Paint devolveu {type(textured)}, esperado Trimesh")

    return textured


def paint_file_to_file(
    mesh_path: Union[str, Path],
    image_path: Union[str, Path],
    output_path: Union[str, Path],
    *,
    model_repo: Optional[str] = None,
    subfolder: Optional[str] = None,
    paint_cpu_offload: Optional[bool] = None,
    verbose: bool = False,
) -> Path:
    """Atalho: carrega mesh, pinta, exporta GLB."""
    repo = model_repo or _defaults.DEFAULT_PAINT_HF_REPO
    sub = subfolder or _defaults.DEFAULT_PAINT_SUBFOLDER
    offload = (
        _defaults.DEFAULT_PAINT_CPU_OFFLOAD if paint_cpu_offload is None else paint_cpu_offload
    )

    mesh = load_mesh_trimesh(mesh_path)
    out = apply_hunyuan_paint(
        mesh,
        image_path,
        model_repo=repo,
        subfolder=sub,
        paint_cpu_offload=offload,
        verbose=verbose,
    )
    output_path = Path(output_path)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    out.export(str(output_path), file_type="glb")
    return output_path
