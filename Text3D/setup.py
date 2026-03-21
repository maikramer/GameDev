"""
Text3D - Setup Script
Text-to-3D com Text2D + Hunyuan3D-2mini (hy3dgen)
"""

from setuptools import setup, find_packages
import os

# Read files from project root
here = os.path.abspath(os.path.dirname(__file__))

with open(os.path.join(here, "README.md"), "r", encoding="utf-8") as fh:
    long_description = fh.read()

with open(os.path.join(here, "config", "requirements.txt"), "r", encoding="utf-8") as fh:
    requirements = [line.strip() for line in fh if line.strip() and not line.startswith("#")]

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
    entry_points={
        "console_scripts": [
            "text3d=text3d.cli:main",
        ],
    },
)
