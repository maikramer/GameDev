"""
Text3D - Setup Script
Text-to-3D com Text2D + Hunyuan3D-2mini (hy3dgen)
"""

from pathlib import Path

from setuptools import find_packages, setup

# Read files from project root
here = Path(__file__).resolve().parent

with open(here / "README.md", encoding="utf-8") as fh:
    long_description = fh.read()

# Monorepo: Text2D e Shared ao lado de Text3D (GameDev/Text2D, GameDev/Shared).
# Caminho absoluto evita falha quando `pip install -e` corre com cwd fora de Text3D.
text2d_local = (here.parent / "Text2D").resolve()
shared_local = (here.parent / "Shared").resolve()
requirements = []
with open(here / "config" / "requirements.txt", encoding="utf-8") as fh:
    for line in fh:
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("text2d @ file:"):
            requirements.append(f"text2d @ {text2d_local.as_uri()}")
        elif line.startswith("gamedev-shared @ file:"):
            requirements.append(f"gamedev-shared @ {shared_local.as_uri()}")
        else:
            requirements.append(line)

setup(
    name="text3d",
    version="0.1.0",
    author="Text3D Project",
    description="Text-to-3D com Text2D + Hunyuan3D-2mini (pouca VRAM)",
    long_description=long_description,
    long_description_content_type="text/markdown",
    url="https://github.com/seu-usuario/text3d",
    package_dir={"": "src"},
    packages=find_packages(where="src"),
    package_data={"text3d": ["cursor_skill/*.md"]},
    classifiers=[
        "Development Status :: 3 - Alpha",
        "Intended Audience :: Developers",
        "Topic :: Multimedia :: Graphics :: 3D Modeling",
        "License :: OSI Approved :: MIT License",
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3.8",
        "Programming Language :: Python :: 3.9",
        "Programming Language :: Python :: 3.10",
        "Programming Language :: Python :: 3.11",
    ],
    python_requires=">=3.8",
    install_requires=requirements,
    extras_require={
        "dev": ["pytest>=7.4.0"],
    },
    entry_points={
        "console_scripts": [
            "text3d=text3d.cli:main",
        ],
    },
)
