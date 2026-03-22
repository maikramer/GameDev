#!/usr/bin/env bash
# Ativa o .venv do GameAssets e executa um comando opcional (como Text2D).
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=/dev/null
source "$SCRIPT_DIR/.venv/bin/activate"
exec "$@"
