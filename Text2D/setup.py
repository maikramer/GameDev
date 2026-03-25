"""Text2D - Setup Script — Text-to-2D com FLUX.2 Klein (SDNQ)."""

import os
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
    name="text2d",
    version="0.1.0",
    author="Text2D Project",
    description="CLI Text-to-2D com FLUX.2 Klein 4B (SDNQ) para GPUs modestas",
    long_description=long_description,
    long_description_content_type="text/markdown",
    url="https://github.com/seu-usuario/text2d",
    package_dir={"": "src"},
    packages=find_packages(where="src"),
    package_data={"text2d": ["cursor_skill/*.md"]},
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
        "dev": ["pytest>=7.4.0"],
    },
    entry_points={
        "console_scripts": [
            "text2d=text2d.cli:main",
        ],
    },
)
