# Estrutura do Projeto Text3D

```
text3d/                                    # Raiz do projeto
│
├── 📁 src/text3d/                         # Código fonte principal
│   ├── __init__.py                         # Inicialização do pacote
│   ├── __main__.py                         # Entry point: python -m text3d
│   ├── cli.py                              # Interface de linha de comando
│   ├── generator.py                        # HunyuanTextTo3DGenerator (Text2D + Hunyuan)
│   │
│   └── 📁 utils/                           # Utilitários
│       ├── __init__.py
│       ├── memory.py                       # Gerenciamento de GPU/VRAM
│       └── export.py                       # Exportação 3D (GLB, PLY, OBJ)
│
├── 📁 scripts/                             # Scripts de instalação
│   ├── setup.sh                            # Setup com venv (desenvolvimento)
│   ├── installer.py                        # Instalador system-wide (Python)
│   └── install.sh                          # Wrapper shell para installer.py
│
├── 📁 config/                              # Configurações e dependências
│   └── requirements.txt                    # Dependências Python
│
├── 📁 docs/                                # Documentação
│   ├── SKILL.md                            # Guia para IA/Agentes
│   └── README.md                           # Documentação principal
│
├── 📁 tests/                               # Testes (para implementação futura)
│
├── 📁 outputs/                             # Saída de modelos gerados
│   ├── meshes/                             # Arquivos 3D (.glb, .ply, .obj)
│   ├── gifs/                               # Previews animados
│   └── images/                             # Imagens intermediárias
│
├── 📁 models/                              # (opcional) pesos locais HF
│
├── setup.py                                # Configuração do pacote Python
├── LICENSE                                 # Licença MIT
├── README.md                               # README da raiz (link para docs/)
└── TREE.md                                 # Este arquivo

# Diretórios gerados automaticamente (não versionar)
├── 📁 .venv/                               # Ambiente virtual (setup.sh)
├── 📁 .git/                                # Repositório git
└── 📁 text3d.egg-info/                   # Metadados do pacote
```

## Convenções de Organização

### Código Fonte (`src/`)
- Todo o código Python do pacote fica em `src/text3d/`
- Facilita instalação e testes
- Separa código de configuração

### Scripts (`scripts/`)
- Scripts executáveis (bash, python)
- Instaladores e ferramentas de setup
- Independentes do código fonte

### Configuração (`config/`)
- Arquivos de configuração
- requirements.txt e similares
- Separado do código

### Documentação (`docs/`)
- SKILL.md para IA/Agentes
- README.md para usuários
- Manuais e guias

### Outputs (`outputs/`)
- Resultados da geração 3D
- Criado automaticamente
- Não versionar (adicionar ao .gitignore)

## Fluxo de Instalação

1. **Desenvolvimento**: `scripts/setup.sh` → cria `.venv/` → instala em modo editable
2. **System-wide**: `scripts/installer.py` → instala no sistema (ex: `~/.local` ou `/usr/local`)
3. **IA/Automação**: `python3 scripts/installer.py --use-venv --skip-deps`

## Uso

```bash
# Após instalação, o comando text3d estará disponível
text3d generate "um robo futuristico" --output robo.glb

# Ou via venv (desenvolvimento)
source .venv/bin/activate
text3d generate "um carro" --output carro.glb
```
