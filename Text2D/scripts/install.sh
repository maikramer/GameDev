#!/bin/bash
# Atalho local: delega para run_installer.sh.
# NÃO é o instalador unificado do monorepo — esse é ../../install.sh na raiz GameDev.
set -e
exec "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/run_installer.sh" "$@"
