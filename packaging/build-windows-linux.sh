#!/bin/sh
# Builds the Windows and Linux packages into dist/. Needs Docker to be running.
set -eu
cd "$(dirname "$0")/.."

RUST_VERSION=$(sed -n 's/^channel = "\(.*\)"/\1/p' rust-toolchain.toml)

docker build --build-arg RUST_VERSION="$RUST_VERSION" -t pigeon-builder packaging/docker
docker run --rm \
    -v "$PWD":/src -w /src \
    -v pigeon-cargo-registry:/usr/local/cargo/registry \
    -v pigeon-target:/target -e CARGO_TARGET_DIR=/target \
    pigeon-builder sh packaging/docker/package.sh

echo
echo "Done:"
ls -lh dist
