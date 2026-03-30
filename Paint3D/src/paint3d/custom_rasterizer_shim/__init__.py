"""
Drop-in replacement for Hunyuan3D-2's ``custom_rasterizer`` CUDA extension.

Uses **nvdiffrast** (NVIDIA, pip-installable) instead of requiring a manual
sparse-clone + CUDA compilation.  The public API exposes the same ``rasterize``
and ``interpolate`` functions that ``hy3dgen.texgen.differentiable_renderer.mesh_render``
expects.
"""

from __future__ import annotations

from .render import interpolate, rasterize

IS_NVDIFFRAST_SHIM = True

__all__ = ["IS_NVDIFFRAST_SHIM", "interpolate", "rasterize"]
