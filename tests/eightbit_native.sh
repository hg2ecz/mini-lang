#!/usr/bin/env bash
# Compatibility entry point for the original 3.5 smoke command.
set -euo pipefail
exec bash "$(dirname "$0")/eightbit36.sh"
