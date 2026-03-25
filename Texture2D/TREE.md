# Texture2D — Estrutura do Projeto

```
Texture2D/
├── config/
│   ├── requirements.txt          # Dependências de runtime
│   └── requirements-dev.txt      # Dependências de desenvolvimento
├── docs/                         # Documentação extra
├── scripts/
│   ├── setup.sh                  # Cria venv + instala deps
│   ├── install.sh                # Wrapper bash do instalador
│   └── installer.py              # Instalador system-wide
├── src/
│   └── texture2d/
│       ├── __init__.py           # Versão do pacote
│       ├── __main__.py           # python -m texture2d
│       ├── cli.py                # CLI principal (Click + Rich)
│       ├── cli_rich.py           # Configuração rich-click
│       ├── generator.py          # TextureGenerator (core HF Inference)
│       ├── presets.py            # Presets de materiais
│       ├── utils.py              # Validação, seeds, helpers
│       ├── image_processor.py    # save_image, ZIP, metadata JSON
│       ├── cursor_skill_install.py
│       └── cursor_skill/
│           └── SKILL.md          # Agent Skill Cursor
├── tests/
│   ├── test_generator.py
│   ├── test_presets.py
│   └── test_utils.py
├── setup.py                      # setuptools (pip install -e .)
├── README.md
├── LICENSE
├── activate.sh
├── pytest.ini
└── TREE.md                       # Este ficheiro
```
