#!/usr/bin/env python3
"""
Text3D System-Wide Installer (Python)
Instalação automatizada para uso por IA e desenvolvedores
"""

import os
import sys
import argparse
import subprocess
import shutil
from pathlib import Path
from typing import Optional, List, Tuple


class Colors:
    """Cores para terminal"""
    GREEN = '\033[0;32m'
    YELLOW = '\033[1;33m'
    RED = '\033[0;31m'
    BLUE = '\033[0;34m'
    NC = '\033[0m'


class Logger:
    """Logger com cores"""
    @staticmethod
    def info(msg: str):
        print(f"{Colors.GREEN}[INFO]{Colors.NC} {msg}")
    
    @staticmethod
    def warn(msg: str):
        print(f"{Colors.YELLOW}[WARN]{Colors.NC} {msg}")
    
    @staticmethod
    def error(msg: str):
        print(f"{Colors.RED}[ERROR]{Colors.NC} {msg}")
    
    @staticmethod
    def step(msg: str):
        print(f"{Colors.BLUE}[STEP]{Colors.NC} {msg}")


class Text3DInstaller:
    """Instalador system-wide do Text3D"""
    
    def __init__(self, args: argparse.Namespace):
        self.args = args
        # Diretório do script é scripts/, mas precisamos do projeto raiz
        script_dir = Path(__file__).parent.resolve()
        self.script_dir = script_dir.parent  # Sobe um nível para a raiz do projeto
        self.venv_dir = self.script_dir / ".venv"
        
        # Configurações
        self.install_prefix = Path(args.prefix)
        self.python_cmd = args.python
        self.use_venv = args.use_venv
        self.skip_deps = args.skip_deps
        self.skip_models = args.skip_models
        self.skip_env_config = args.skip_env_config
        self.force = args.force
        
        self.requirements_path = self.script_dir / "config" / "requirements.txt"
        
        # Detectar venv existente
        self.venv_python = self.venv_dir / "bin" / "python" if self.venv_dir.exists() else None
        self.venv_exists = self.venv_python and self.venv_python.exists()
        
        self.logger = Logger()
    
    def run(self) -> bool:
        """Executa a instalação completa"""
        self.logger.info(f"Prefixo de instalação: {self.install_prefix}")
        self.logger.info(f"Python: {self.python_cmd}")
        
        # Verificações
        if not self.check_python():
            return False
        
        if not self.skip_deps:
            self.install_system_deps()
        
        # Instalar Text3D (venv ou system-wide)
        if self.use_venv and self.venv_exists:
            self.logger.info(f"Usando venv existente: {self.venv_dir}")
            self.install_in_venv()
        else:
            self.install_system_wide()
        
        # Configurar modelos
        self.setup_models()
        
        # Criar wrappers
        self.create_wrappers()
        
        # Setup diretórios
        self.setup_directories()
        
        if not self.skip_env_config:
            self.write_env_file()
        
        # Resumo
        self.show_summary()
        
        return True
    
    def check_python(self) -> bool:
        """Verifica versão do Python"""
        self.logger.step("Verificando Python...")
        
        try:
            result = subprocess.run(
                [self.python_cmd, "--version"],
                capture_output=True,
                text=True,
                check=True
            )
            version_str = result.stdout.strip() or result.stderr.strip()
            self.logger.info(f"Python detectado: {version_str}")
            
            # Verificar versão mínima
            version_ok = subprocess.run(
                [self.python_cmd, "-c", 
                 "import sys; print('OK' if sys.version_info >= (3, 8) else 'FAIL')"],
                capture_output=True,
                text=True,
                check=True
            )
            
            if "OK" not in version_ok.stdout:
                self.logger.error("Python 3.8+ necessário")
                return False
            
            return True
            
        except (subprocess.CalledProcessError, FileNotFoundError) as e:
            self.logger.error(f"Python não encontrado: {e}")
            return False
    
    def install_system_deps(self):
        """Instala dependências do sistema"""
        self.logger.step("Verificando dependências do sistema...")
        
        # Detectar sistema
        if shutil.which("apt-get"):
            self.logger.info("Detectado: Debian/Ubuntu")
            # Não tentar instalar sem sudo
            self.logger.warn("Para dependências do sistema (base + compilar Paint/custom_rasterizer):")
            self.logger.info(
                "  sudo apt-get install python3-dev python3-pip git "
                "build-essential nvidia-cuda-toolkit nvidia-cuda-dev"
            )
        elif shutil.which("dnf"):
            self.logger.info("Detectado: Fedora")
            self.logger.warn("Para instalar dependências do sistema:")
            self.logger.info("  sudo dnf install python3-devel python3-pip git")
        elif shutil.which("pacman"):
            self.logger.info("Detectado: Arch Linux")
            self.logger.warn("Para instalar dependências do sistema:")
            self.logger.info("  sudo pacman -S python python-pip git base-devel")
        else:
            self.logger.warn("Gerenciador de pacotes não reconhecido")
    
    def _warn_monorepo_layout(self) -> None:
        """requirements inclui text2d @ file:../Text2D — pastas irmãs no monorepo."""
        text2d = self.script_dir.parent / "Text2D"
        if not text2d.is_dir():
            self.logger.warn(
                "Monorepo: espera-se Text2D ao lado de Text3D (ex.: GameDev/Text2D + GameDev/Text3D)."
            )
            self.logger.warn(
                f"Não encontrado: {text2d} — `pip install .` pode falhar em text2d."
            )
    
    def install_in_venv(self):
        """Instala no venv existente (setup.py lê config/requirements.txt)."""
        self.logger.step("Instalando no venv existente...")
        
        python = str(self.venv_python)
        
        self._warn_monorepo_layout()
        
        # Verificar se já está instalado
        if not self.force:
            try:
                subprocess.run(
                    [python, "-c", "import text3d"],
                    capture_output=True,
                    check=True
                )
                self.logger.warn("Text3D já instalado no venv")
                self.logger.info("Use --force para reinstalar")
                return
            except subprocess.CalledProcessError:
                pass
        
        pip_cmd = [python, "-m", "pip", "install"]
        extra = ["--force-reinstall"] if self.force else []
        
        self.logger.info(
            f"Instalando a partir de {self.script_dir} (deps: {self.requirements_path.name})..."
        )
        subprocess.run(
            pip_cmd + extra + ["."],
            cwd=str(self.script_dir),
            check=True,
        )
        
        self.logger.info("✓ Instalado no venv")
    
    def install_system_wide(self):
        """Instala system-wide com PyTorch (deps via setup.py → config/requirements.txt)."""
        self.logger.step("Instalando Text3D system-wide...")
        
        pip_cmd = [self.python_cmd, "-m", "pip", "install"]
        
        self._warn_monorepo_layout()
        
        # Atualizar pip
        subprocess.run(
            [self.python_cmd, "-m", "pip", "install", "--upgrade", 
             "pip", "setuptools", "wheel"],
            check=True
        )
        
        # Instalar PyTorch com CUDA se disponível (antes do resto das deps)
        self.install_pytorch()
        
        if not self.requirements_path.exists():
            self.logger.warn(
                f"Ficheiro em falta: {self.requirements_path} — a instalar só o pacote."
            )
        
        self.logger.info("Instalando pacote text3d e dependências...")
        subprocess.run(
            pip_cmd + ["."],
            cwd=str(self.script_dir),
            check=True,
        )
        
        self.logger.info("✓ Instalado system-wide")
    
    def install_pytorch(self):
        """Instala PyTorch com suporte CUDA se disponível"""
        # Detectar CUDA
        has_cuda = shutil.which("nvidia-smi") is not None
        
        pip_cmd = [self.python_cmd, "-m", "pip", "install"]
        
        if has_cuda:
            try:
                result = subprocess.run(
                    ["nvidia-smi"],
                    capture_output=True,
                    text=True
                )
                if "CUDA Version" in result.stdout:
                    # Extrair versão CUDA
                    for line in result.stdout.split("\n"):
                        if "CUDA Version" in line:
                            cuda_version = line.split("CUDA Version:")[1].split()[0]
                            self.logger.info(f"CUDA detectado: {cuda_version}")
                            
                            if cuda_version.startswith("12"):
                                self.logger.info("Instalando PyTorch com CUDA 12.1...")
                                subprocess.run(
                                    pip_cmd + ["torch", "torchvision", 
                                              "--index-url", "https://download.pytorch.org/whl/cu121"],
                                    check=True
                                )
                            else:
                                self.logger.info("Instalando PyTorch com CUDA 11.8...")
                                subprocess.run(
                                    pip_cmd + ["torch", "torchvision",
                                              "--index-url", "https://download.pytorch.org/whl/cu118"],
                                    check=True
                                )
                            return
            except Exception:
                pass
        
        # CPU fallback
        self.logger.warn("Instalando PyTorch para CPU...")
        subprocess.run(
            pip_cmd + ["torch", "torchvision",
                      "--index-url", "https://download.pytorch.org/whl/cpu"],
            check=True
        )
    
    def setup_models(self):
        """Hunyuan3D-2mini + Text2D: cache Hugging Face (~/.cache/huggingface)."""
        self.logger.step("Configurando modelos...")
        
        hf_cache = Path.home() / ".cache" / "huggingface"
        models_dir = self.script_dir / "models"
        
        if not self.skip_models:
            self.logger.info(
                "Text2D (FLUX/SDNQ) e Hunyuan3D-2mini vêm do Hugging Face na primeira execução."
            )
            self.logger.info(f"Cache típico: {hf_cache}")
            self.logger.info("Opcional: huggingface-cli login (modelos gated / quotas)")
        
        if models_dir.exists() and any(models_dir.iterdir()):
            self.logger.info(f"Pasta local opcional (assets): {models_dir}")
        
        config_dir = Path.home() / ".config" / "text3d"
        config_dir.mkdir(parents=True, exist_ok=True)
        config_file = config_dir / "config.env"
        if not config_file.exists():
            with open(config_file, "w", encoding="utf-8") as f:
                f.write("# Text3D — gerado por scripts/installer.py\n")
                f.write(f"TEXT3D_OUTPUT_DIR={Path.home() / '.text3d/outputs'}\n")
                if models_dir.exists():
                    f.write(f"TEXT3D_MODELS_DIR={models_dir}\n")
            self.logger.info(f"Config criada: {config_file}")
        else:
            self.logger.info(f"Config existente (mantida): {config_file}")
    
    def write_env_file(self) -> None:
        """Snippet shell: PYTORCH_CUDA_ALLOC_CONF (reduz fragmentação CUDA)."""
        config_dir = Path.home() / ".config" / "text3d"
        config_dir.mkdir(parents=True, exist_ok=True)
        env_sh = config_dir / "env.sh"
        content = """# Text3D — gerado por scripts/installer.py
# source ~/.config/text3d/env.sh
export PYTORCH_CUDA_ALLOC_CONF="${PYTORCH_CUDA_ALLOC_CONF:-expandable_segments:True}"

# Descomenta se compilaste custom_rasterizer com um toolkit específico:
# export CUDA_HOME=/usr/local/cuda-11.8
"""
        with open(env_sh, "w", encoding="utf-8") as f:
            f.write(content)
        self.logger.info(f"Ambiente opcional: {env_sh}")
    
    def create_wrappers(self):
        """Cria wrappers executáveis"""
        self.logger.step("Criando wrappers...")
        
        bin_dir = self.install_prefix / "bin"
        bin_dir.mkdir(parents=True, exist_ok=True)
        
        # Determinar qual Python usar
        if self.venv_exists and self.use_venv:
            python_path = str(self.venv_python)
        else:
            python_path = self.python_cmd
        
        # Wrapper text3d
        wrapper = bin_dir / "text3d"
        env_sh = Path.home() / ".config" / "text3d" / "env.sh"
        with open(wrapper, "w") as f:
            f.write("#!/bin/bash\n")
            f.write("# Text3D wrapper — gerado por scripts/installer.py\n")
            f.write(f'if [[ -f "{env_sh}" ]]; then\n')
            f.write(f'  # shellcheck source=/dev/null\n')
            f.write(f'  . "{env_sh}"\n')
            f.write("fi\n")
            f.write(f'exec {python_path} -m text3d.cli "$@"\n')
        
        wrapper.chmod(0o755)
        self.logger.info(f"✓ Wrapper criado: {wrapper}")
        
        # Wrapper text3d-generate
        wrapper_gen = bin_dir / "text3d-generate"
        with open(wrapper_gen, "w") as f:
            f.write(f"#!/bin/bash\n")
            f.write(f"# Text3D quick generate\n")
            f.write(f'exec {bin_dir}/text3d generate "$@"\n')
        wrapper_gen.chmod(0o755)
        
        # Helper text3d-activate (se usar venv)
        if self.venv_exists and self.use_venv:
            wrapper_act = bin_dir / "text3d-activate"
            with open(wrapper_act, "w") as f:
                f.write(f"#!/bin/bash\n")
                f.write(f"# Activate Text3D environment\n")
                f.write(f"source {self.venv_dir}/bin/activate\n")
                f.write('exec "$@"\n')
            wrapper_act.chmod(0o755)
            self.logger.info(f"✓ Helper criado: text3d-activate")
    
    def setup_directories(self):
        """Cria diretórios de saída"""
        output_dir = Path.home() / ".text3d" / "outputs"
        
        (output_dir / "meshes").mkdir(parents=True, exist_ok=True)
        (output_dir / "gifs").mkdir(parents=True, exist_ok=True)
        (output_dir / "images").mkdir(parents=True, exist_ok=True)
        
        self.logger.info(f"Diretórios de saída: {output_dir}")
    
    def show_summary(self):
        """Mostra resumo da instalação"""
        print("\n" + "=" * 42)
        print(f"{Colors.GREEN}  Instalação Concluída!{Colors.NC}")
        print("=" * 42)
        print()
        
        if self.venv_exists and self.use_venv:
            print(f"✓ Usando venv existente: {self.venv_dir}")
            print()
        
        env_sh = Path.home() / ".config" / "text3d" / "env.sh"
        if not self.skip_env_config and env_sh.exists():
            print("Ambiente CUDA (opcional no teu shell):")
            print(f"  source {env_sh}")
            print("  (o wrapper em ~/.local/bin/text3d já tenta carregar este ficheiro)")
            print()
        
        print("Comandos úteis:")
        print("  text3d doctor              # GPU, torch, hy3dgen, custom_rasterizer")
        print("  text3d --help")
        print("  text3d generate 'um robô' -o robô.glb --preset balanced")
        print("  text3d generate 'prompt' --final -o modelo.glb   # + textura (Paint)")
        print("  text3d texture mesh.glb -i ref.png -o pintado.glb")
        print()
        
        print("Modelos: cache Hugging Face (~/.cache/huggingface); primeira execução descarrega.")
        print()
        
        paint_doc = self.script_dir / "docs" / "PAINT_SETUP.md"
        raster_sh = self.script_dir / "scripts" / "install_custom_rasterizer.sh"
        print("Hunyuan3D-Paint (text3d texture / --final) precisa de custom_rasterizer (CUDA):")
        print(f"  Ver: {paint_doc}")
        if raster_sh.exists():
            print(f"  Build: bash {raster_sh}")
        print()
        
        print(f"  Saída: {Path.home() / '.text3d/outputs'}")
        print(f"  Binários: {self.install_prefix}/bin/")
        print()
        
        text3d_path = shutil.which("text3d")
        if text3d_path:
            print(f"✓ text3d no PATH: {text3d_path}")
        else:
            print(f'⚠ Adiciona ao PATH: export PATH="{self.install_prefix}/bin:$PATH"')
        
        print()
        print("Exemplo rápido:")
        print("  text3d doctor && text3d generate 'um carro' --preset fast -o carro.glb")
        print()


def main():
    """Entry point"""
    parser = argparse.ArgumentParser(
        description="Text3D System-Wide Installer",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Exemplos:
  # Instalação usando venv existente (rápido)
  %(prog)s --use-venv
  
  # Instalação system-wide completa
  sudo %(prog)s --prefix /usr/local
  
  # Instalação local
  %(prog)s --prefix ~/.local
  
  # Rápido, sem deps/modelos
  %(prog)s --use-venv --skip-deps --skip-models

Variáveis de ambiente:
  INSTALL_PREFIX    Diretório de instalação
  TEXT3D_OUTPUT_DIR Diretório de saída padrão
  TEXT3D_MODELS_DIR Diretório de modelos (opcional)

Pós-instalação:
  - source ~/.config/text3d/env.sh  (PYTORCH_CUDA_ALLOC_CONF)
  - text3d doctor
  - Para Paint: docs/PAINT_SETUP.md e scripts/install_custom_rasterizer.sh
        """
    )
    
    parser.add_argument(
        "--prefix",
        default=os.environ.get("INSTALL_PREFIX", str(Path.home() / ".local")),
        help="Diretório de instalação (padrão: ~/.local)"
    )
    parser.add_argument(
        "--use-venv",
        action="store_true",
        help="Usar venv existente no diretório do projeto"
    )
    parser.add_argument(
        "--skip-deps",
        action="store_true",
        help="Pular verificação de dependências do sistema"
    )
    parser.add_argument(
        "--skip-models",
        action="store_true",
        help="Pular configuração de modelos"
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Forçar reinstalação"
    )
    parser.add_argument(
        "--skip-env-config",
        action="store_true",
        help="Não escrever ~/.config/text3d/env.sh (PYTORCH_CUDA_ALLOC_CONF)"
    )
    parser.add_argument(
        "--python",
        default=os.environ.get("PYTHON_CMD", "python3"),
        help="Comando Python a usar (padrão: python3)"
    )
    
    args = parser.parse_args()
    
    # Raiz do projeto Text3D = scripts/..
    venv_dir = Path(__file__).resolve().parent.parent / ".venv"
    if venv_dir.exists() and not args.use_venv:
        print(f"{Colors.YELLOW}[INFO]{Colors.NC} Venv detectado em: {venv_dir}")
        print(f"{Colors.YELLOW}[INFO]{Colors.NC} Use --use-venv para instalação mais rápida")
        print()
    
    # Executar instalação
    installer = Text3DInstaller(args)
    success = installer.run()
    
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()
