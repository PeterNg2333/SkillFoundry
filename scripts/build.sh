#!/bin/sh
set -eu

repository=$(CDPATH= cd -P "$(dirname "$0")/.." && pwd)
cd "$repository/checker"
host=$(rustc -vV | sed -n 's/^host: //p' | tr -d '\r')
target=${CHECKER_TARGET:-$host}

cargo fmt --all -- --check
cargo test --locked --target "$host"
cargo clippy --locked --all-targets --target "$host" -- -D warnings
cargo build --locked --release --target "$target"

binary=maintainable-check
case "$target" in *windows*) binary=$binary.exe ;; esac
mkdir -p "$repository/skills/maintainable-code/bin"
cp "target/$target/release/$binary" "$repository/skills/maintainable-code/bin/$binary"
printf 'Built %s\n' "$repository/skills/maintainable-code/bin/$binary"
