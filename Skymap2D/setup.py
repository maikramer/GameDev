"""Skymap2D — Setup Script — Skymaps equirectangular 360° via HF Inference API."""

from pathlib import Path

from setuptools import find_packages, setup

here = Path(__file__).resolve().parent

with open(here / "README.md", "r", encoding="utf-8") as fh:
    long_description = fh.read()

shared_local = (here.parent / "Shared").resolve()
requirements = []
with open(here / "config" / "requirements.txt", "r", encoding="utf-8") as fh:
    for line in fh:
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("gamedev-shared @ file:"):
            requirements.append(f"gamedev-shared @ {shared_local.as_uri()}")
        else:
            requirements.append(line)

setup(
    name="skymap2d",
    version="0.1.0",
    author="Skymap2D Project",
    description="CLI para geração de skymaps equirectangular 360° via HF Inference API (Flux LoRA)",
    long_description=long_description,
    long_description_content_type="text/markdown",
    url="https://github.com/seu-usuario/skymap2d",
    package_dir={"": "src"},
    packages=find_packages(where="src"),
    package_data={"skymap2d": ["cursor_skill/*.md"]},
    classifiers=[
        "Development Status :: 3 - Alpha",
        "Intended Audience :: Developers",
        "Topic :: Multimedia :: Graphics",
        "License :: OSI Approved :: MIT License",
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3.10",
        "Programming Language :: Python :: 3.11",
        "Programming Language :: Python :: 3.12",
        "Programming Language :: Python :: 3.13",
    ],
    python_requires=">=3.10",
    install_requires=requirements,
    extras_require={
        "dev": ["pytest>=7.4.0", "pytest-cov>=4.0.0"],
    },
    entry_points={
        "console_scripts": [
            "skymap2d=skymap2d.cli:main",
        ],
    },
)
