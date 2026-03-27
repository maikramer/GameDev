"""GameAssets — setup."""

from pathlib import Path

from setuptools import find_packages, setup

here = Path(__file__).resolve().parent

shared_local = (here.parent / "Shared").resolve()
requirements = []
with open(here / "config" / "requirements.txt", encoding="utf-8") as fh:
    for line in fh:
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("gamedev-shared @ file:"):
            requirements.append(f"gamedev-shared @ {shared_local.as_uri()}")
        else:
            requirements.append(line)

with open(here / "config" / "requirements-dev.txt", encoding="utf-8") as fh:
    dev_requirements = []
    for line in fh:
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        # pip -r include; not valid in setuptools extras (PEP 508)
        if line.startswith("-r ") or line.startswith("--requirement "):
            continue
        dev_requirements.append(line)

setup(
    # name/version/description/readme: declarados em pyproject.toml ([project])
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
