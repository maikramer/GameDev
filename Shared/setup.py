"""gamedev-shared — biblioteca partilhada do monorepo GameDev."""

import os

from setuptools import find_packages, setup

here = os.path.abspath(os.path.dirname(__file__))

with open(os.path.join(here, "README.md"), "r", encoding="utf-8") as fh:
    long_description = fh.read()

with open(os.path.join(here, "config", "requirements.txt"), "r", encoding="utf-8") as fh:
    requirements = [
        line.strip()
        for line in fh
        if line.strip() and not line.startswith("#")
    ]

setup(
    name="gamedev-shared",
    version="0.1.0",
    author="GameDev",
    description="Biblioteca partilhada do monorepo GameDev (logging, GPU, instaladores, CLI, subprocess)",
    long_description=long_description,
    long_description_content_type="text/markdown",
    package_dir={"": "src"},
    packages=find_packages(where="src"),
    python_requires=">=3.10",
    install_requires=requirements,
    extras_require={
        "gpu": ["torch>=2.1.0"],
        "cli": ["click>=8.1.0", "rich-click>=1.8.0"],
        "dev": ["pytest>=7.4.0"],
    },
    entry_points={
        "console_scripts": [
            "gamedev-install=gamedev_shared.installer.unified:main",
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
