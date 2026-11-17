#!/bin/sh

find_python() {
  for candidate in python python3; do
    if command -v "$candidate" >/dev/null 2>&1 && \
       "$candidate" -c 'import sys; sys.exit(sys.version_info < (3, 10))' >/dev/null 2>&1; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done
  printf 'Python 3.10 or newer is required for ZIP packaging and installation.\n' >&2
  return 1
}
