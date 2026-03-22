#!/usr/bin/env python3
"""
Materialize CLI Installer - Cross-platform installation
Style: Python installer, Rust binary (similar to denv-cli / galaxy).
"""

import os
import sys
import shutil
import subprocess
import platform
from pathlib import Path
from typing import Optional

try:
    from rich import box
    from rich.console import Console
    from rich.panel import Panel
    from rich.table import Table

    _RICH = True
    _console = Console()
except ImportError:
    _RICH = False
    _console = None

# Nome do binário no Cargo (target/release/materialize-cli)
CARGO_BIN_NAME = "materialize-cli"
# Nome do comando no PATH
CLI_NAME = "materialize"


class Installer:
    """Cross-platform installer for Materialize CLI (Rust)."""

    def __init__(self):
        # Diretório do instalador (installer/) e raiz do repo (Materialize-CLI/)
        self.installer_dir = Path(__file__).parent.resolve()
        self.repo_dir = self.installer_dir.parent.resolve()
        self.platform = platform.system().lower()
        self.is_windows = self.platform == 'windows'
        self.is_macos = self.platform == 'darwin'
        self.is_linux = self.platform == 'linux'

        # Caminho do binário após build
        self.release_binary = self.repo_dir / "target" / "release" / CARGO_BIN_NAME
        self.debug_binary = self.repo_dir / "target" / "debug" / CARGO_BIN_NAME

        # Diretório de instalação (binários no PATH)
        if self.is_windows:
            user_profile = os.environ.get('USERPROFILE') or 'C:\\'
            self.bin_dir = Path(user_profile) / 'bin'
        else:
            self.bin_dir = Path.home() / '.local' / 'bin'

        self.installed_binary = self.bin_dir / CLI_NAME

    def print_header(self, text: str) -> None:
        """Secção destacada (Rich ou ANSI)."""
        if _RICH and _console:
            _console.print()
            _console.print(
                Panel(
                    f"[bold cyan]{text}[/bold cyan]",
                    border_style="cyan",
                    expand=False,
                )
            )
        else:
            print(f"\n\033[1m\033[96m{text}\033[0m")
            print("=" * len(text))

    def print_success(self, text: str) -> None:
        if _RICH and _console:
            _console.print(f"[bold green]✓[/bold green] {text}")
        else:
            print(f"\033[92m✓ {text}\033[0m")

    def print_error(self, text: str) -> None:
        if _RICH and _console:
            _console.print(f"[bold red]✗[/bold red] {text}")
        else:
            print(f"\033[91m✗ {text}\033[0m")

    def print_warning(self, text: str) -> None:
        if _RICH and _console:
            _console.print(f"[bold yellow]⚠[/bold yellow] {text}")
        else:
            print(f"\033[93m⚠ {text}\033[0m")

    def print_info(self, text: str) -> None:
        if _RICH and _console:
            _console.print(f"[bold blue]ℹ[/bold blue] {text}")
        else:
            print(f"\033[94mℹ {text}\033[0m")

    def check_python(self) -> bool:
        """Check Python 3 (required to run this installer)."""
        self.print_header("Checking dependencies")
        try:
            result = subprocess.run(
                [sys.executable, "--version"],
                capture_output=True,
                text=True,
                check=True,
                timeout=5,
            )
            self.print_success(f"Python: {result.stdout.strip()}")
            return True
        except Exception as e:
            self.print_error(f"Python not found: {e}")
            return False

    def check_cargo(self) -> bool:
        """Check if Rust/cargo is installed."""
        try:
            result = subprocess.run(
                ["cargo", "--version"],
                capture_output=True,
                text=True,
                check=True,
                timeout=5,
            )
            self.print_success(f"Rust: {result.stdout.strip()}")
            return True
        except (subprocess.CalledProcessError, FileNotFoundError):
            self.print_warning("cargo not found in PATH")
            return False

    def get_existing_binary(self) -> Optional[Path]:
        """Retorna o caminho do binário se já existir (release ou debug)."""
        if self.release_binary.exists():
            return self.release_binary
        if self.debug_binary.exists():
            return self.debug_binary
        return None

    def build_release(self) -> bool:
        """Run cargo build --release."""
        self.print_header("Building Materialize CLI (release)")
        try:
            subprocess.run(
                ["cargo", "build", "--release"],
                cwd=self.repo_dir,
                check=True,
                timeout=600,
            )
            self.print_success("Build complete: target/release/" + CARGO_BIN_NAME)
            return True
        except subprocess.CalledProcessError as e:
            self.print_error(f"Build failed: {e}")
            return False
        except FileNotFoundError:
            self.print_error("cargo not found. Install Rust: https://rustup.rs")
            return False
        except subprocess.TimeoutExpired:
            self.print_error("Build timed out")
            return False

    def create_bin_dir(self) -> bool:
        """Create binary directory."""
        try:
            self.bin_dir.mkdir(parents=True, exist_ok=True)
            self.print_success(f"Directory: {self.bin_dir}")
            return True
        except Exception as e:
            self.print_error(f"Could not create directory: {e}")
            return False

    def install_binary(self) -> bool:
        """Copy or link the binary to the PATH directory."""
        self.print_header("Installing binary")

        src = self.get_existing_binary()
        if not src:
            if not self.check_cargo():
                return False
            if not self.build_release():
                return False
            src = self.release_binary

        if not src.exists():
            self.print_error(f"Binary not found: {src}")
            return False

        try:
            dest = self.installed_binary
            if dest.exists() or dest.is_symlink():
                dest.unlink()
            shutil.copy2(src, dest)
            dest.chmod(0o755)
            self.print_success(f"{CLI_NAME} installed at {dest}")
            return True
        except Exception as e:
            self.print_error(f"Install error: {e}")
            return False

    def update_path(self) -> bool:
        """Inform about PATH."""
        self.print_header("Configuring PATH")
        bin_str = str(self.bin_dir)
        path_env = os.environ.get('PATH', '')

        if self.is_windows:
            path_sep = ';'
        else:
            path_sep = ':'

        if bin_str in path_env:
            self.print_success(f"{bin_str} is already on PATH")
            return True

        self.print_warning(f"{bin_str} may not be on PATH")
        if not self.is_windows:
            self.print_info("Add to ~/.bashrc or ~/.zshrc:")
            self.print_info(f'  export PATH="{bin_str}:$PATH"')
        else:
            self.print_info(f"Add to system PATH manually: {bin_str}")
        return False

    def test_installation(self) -> bool:
        """Test the installed command."""
        self.print_header("Testing installation")
        try:
            result = subprocess.run(
                [str(self.installed_binary), "--version"],
                capture_output=True,
                text=True,
                timeout=10,
            )
            if result.returncode == 0:
                self.print_success(f"Version: {result.stdout.strip()}")
                return True
            self.print_warning("Test returned non-zero exit code")
            return True
        except Exception as e:
            self.print_warning(f"Could not run test: {e}")
            return True

    def uninstall(self) -> bool:
        """Remove the binary from the install directory."""
        self.print_header("Uninstalling Materialize CLI")
        try:
            for name in [CLI_NAME]:
                p = self.bin_dir / name
                if p.exists():
                    p.unlink()
                    self.print_success(f"Removed: {p}")
            self.print_success("Materialize CLI uninstalled.")
            return True
        except Exception as e:
            self.print_error(f"Uninstall error: {e}")
            return False

    def install(self) -> bool:
        """Run full installation."""
        self.print_header("Materialize CLI Installer")
        if _RICH and _console:
            meta = Table(show_header=False, box=box.SIMPLE)
            meta.add_row("Platform", platform.system())
            meta.add_row("Repository", str(self.repo_dir))
            meta.add_row("Install bin dir", str(self.bin_dir))
            _console.print(Panel(meta, title="[bold]Paths", border_style="dim"))
        else:
            print(f"Platform: {platform.system()}")
            print(f"Repository: {self.repo_dir}")
            print(f"Binaries to: {self.bin_dir}")

        if not self.check_python():
            return False
        if not self.create_bin_dir():
            return False
        if not self.install_binary():
            return False

        self.update_path()
        self.test_installation()

        self.print_header("Installation complete")
        if _RICH and _console:
            lines = [
                "[bold]Usage[/bold]",
                f"  [cyan]{CLI_NAME} texture.png -o ./out/[/cyan]  [dim]# PBR maps[/dim]",
                f"  [cyan]{CLI_NAME} --help[/cyan]",
            ]
            if not self.is_windows:
                lines.extend(
                    [
                        "",
                        f"If [yellow]{CLI_NAME}[/yellow] is not found:",
                        f'  [yellow]export PATH="{self.bin_dir}:$PATH"[/yellow]',
                    ]
                )
            _console.print(
                Panel(
                    "\n".join(lines),
                    title="[bold green]Done",
                    border_style="green",
                )
            )
        else:
            print("\nUsage:")
            print(f"  {CLI_NAME} texture.png -o ./out/   # Generate PBR maps")
            print(f"  {CLI_NAME} --help                   # Help")
            if not self.is_windows:
                print(f"\nIf '{CLI_NAME}' is not found, add to PATH:")
                print(f'  export PATH="{self.bin_dir}:$PATH"')
        return True


def main():
    import argparse

    parser = argparse.ArgumentParser(
        description="Materialize CLI Installer (Rust)",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  ./install.sh              # Install (Linux/macOS)
  python3 installer/installer.py install
  python3 installer/installer.py uninstall
  python3 installer/installer.py reinstall
        """,
    )
    parser.add_argument(
        'action',
        nargs='?',
        default='install',
        choices=['install', 'uninstall', 'reinstall'],
        help='Action (default: install)',
    )
    args = parser.parse_args()

    installer = Installer()

    if args.action == 'install':
        success = installer.install()
    elif args.action == 'uninstall':
        success = installer.uninstall()
    elif args.action == 'reinstall':
        installer.uninstall()
        success = installer.install()
    else:
        success = False

    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()
