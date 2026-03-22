# GameAssets — estrutura

```
GameAssets/
├── activate.sh           # ativa .venv e executa comando (opcional)
├── config/
│   ├── requirements.txt      # runtime (também usado por setup.py)
│   └── requirements-dev.txt  # pytest; extras [dev]
├── scripts/
│   └── setup.sh          # cria .venv, pip install -e .
├── src/gameassets/
│   ├── cli.py
│   ├── data/presets.yaml
│   └── ...
├── tests/
├── README.md
└── setup.py
```

Instalação: `chmod +x scripts/setup.sh && ./scripts/setup.sh` e depois `source .venv/bin/activate`.
