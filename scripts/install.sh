#!/bin/sh
set -eu
script_root=$(CDPATH= cd -P "$(dirname "$0")" && pwd)
. "$script_root/runtime.sh"
python_bin=$(find_python)
exec "$python_bin" "$script_root/install.py" "$@"
