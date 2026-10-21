#!/bin/sh
set -eu

cd "$(dirname "$0")/.."
mkdir -p target/protobuf-bin
GOBIN="$PWD/target/protobuf-bin" go install google.golang.org/protobuf/cmd/protoc-gen-go@v1.36.6
protoc --plugin="protoc-gen-go=$PWD/target/protobuf-bin/protoc-gen-go" \
    --proto_path=ffi/proto --go_out=ffi/go --go_opt=module=inferrum.local/ffi bonus.proto
