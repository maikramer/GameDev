# Estrutura — Text2D

```
Text2D/
├── src/text2d/
│   ├── __init__.py
│   ├── __main__.py          # python -m text2d
│   ├── cli.py               # Click + Rich
│   ├── generator.py         # Flux2KleinPipeline + SDNQ
│   └── utils/
│       ├── __init__.py
│       └── memory.py
├── config/
│   ├── requirements.txt     # runtime
│   └── requirements-dev.txt # runtime + pytest
├── scripts/
│   ├── setup.sh
│   ├── install.sh
│   └── installer.py
├── docs/
│   ├── INSTALL.md
│   └── TROUBLESHOOTING.md
├── tests/
│   └── test_cli.py
├── outputs/images/          # gitignored
├── setup.py
├── activate.sh
├── README.md
├── TREE.md
└── LICENSE
```

## Convenções

- Pacote em `src/text2d/`; entrada `text2d` → `text2d.cli:main`.
- Imagens por defeito em `outputs/images/` se `-o` não for passado.
- Dependências de teste: extra setuptools `dev` ou `requirements-dev.txt`.
