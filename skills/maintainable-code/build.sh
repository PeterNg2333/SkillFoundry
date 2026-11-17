#!/bin/sh
set -eu

skill_root=$(CDPATH= cd -P "$(dirname "$0")" && pwd)
cd "$skill_root"

unformatted=$(gofmt -l assets.go cmd internal)
if [ -n "$unformatted" ]; then
  printf 'Run gofmt on\n%s\n' "$unformatted" >&2
  exit 1
fi

host_os=$(go env GOHOSTOS | tr -d '\r')
host_arch=$(go env GOHOSTARCH | tr -d '\r')
CGO_ENABLED=0 GOOS="$host_os" GOARCH="$host_arch" go test -mod=readonly ./...
CGO_ENABLED=0 GOOS="$host_os" GOARCH="$host_arch" go vet -mod=readonly ./...

target_os=$(go env GOOS | tr -d '\r')
binary_name=maintainable-check
if [ "$target_os" = windows ]; then
  binary_name=$binary_name.exe
fi
mkdir -p bin
CGO_ENABLED=0 go build -mod=readonly -trimpath -buildvcs=false '-ldflags=-s -w' \
  -o "$skill_root/bin/$binary_name" ./cmd/check
printf 'Built %s\n' "$skill_root/bin/$binary_name"
