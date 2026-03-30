from __future__ import annotations


def optional_imports() -> dict[str, bool]:
    """Indica quais pacotes opcionais (extra pipelines) estão importáveis."""
    mapping = [
        ("text2d", "text2d"),
        ("text3d", "text3d"),
        ("skymap2d", "skymap2d"),
        ("texture2d", "texture2d"),
    ]
    out: dict[str, bool] = {}
    for key, mod in mapping:
        try:
            __import__(mod)
            out[key] = True
        except ImportError:
            out[key] = False
    return out


def pipelines_ready() -> bool:
    return all(optional_imports().values())
