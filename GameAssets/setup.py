"""GameAssets — setup."""

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

with open(here / "config" / "requirements-dev.txt", "r", encoding="utf-8") as fh:
    dev_requirements = [
        line.strip()
        for line in fh
        if line.strip() and not line.startswith("#")
    ]

setup(
    name="gameassets",
    version="0.2.2",
    author="GameDev",
    description="CLI para batches de prompts e assets 2D/3D alinhados ao estilo do jogo",
    long_description=long_description,
    long_description_content_type="text/markdown",
    package_dir={"": "src"},
    packages=find_packages(where="src"),
    package_data={"gameassets": ["data/*.yaml", "cursor_skill/*.md"]},
    include_package_data=True,
    python_requires=">=3.10",
    install_requires=requirements,
    extras_require={
        "dev": dev_requirements,
    },
    entry_points={
        "console_scripts": [
            "gameassets=gameassets.cli:main",
        ],
    },
    classifiers=[
        "Development Status :: 3 - Alpha",
        "Intended Audience :: Developers",
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3.10",
        "Programming Language :: Python :: 3.11",
        "Programming Language :: Python :: 3.12",
        "Programming Language :: Python :: 3.13",
    ],
)
