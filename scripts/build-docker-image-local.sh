#!/bin/bash
#
# Usage: ./scripts/build-docker.sh
#
set -e
echo -e "\nStarting cennznet build..."

# Clean build
if [[ $CLEAN_CARGO == 'true' ]]; then
  ./scripts/clean-cargo.sh
fi

# Setup a local $CARGO_HOME and fetch dependencies
./scripts/fetch-dependencies.sh

# Create generic rust-builder image from nightly
NIGHTLY_DATE="$(date +%Y%m%d)"

if [[ "$(docker images -q rust-builder:$NIGHTLY_DATE 2> /dev/null)" == "" ]]; then
  echo "Building rust-builder image..."
  docker build --no-cache --pull -f docker/rust-builder.Dockerfile -t rust-builder:$NIGHTLY_DATE .
else
  echo "rust-builder image for $NIGHTLY_DATE exists. Not rebuilding..."
fi

# Build cennznet-node runtime WASM binary
echo -e "\nBuilding runtime wasm..."
docker run --user "$(id -u)":"$(id -g)" \
      -t --rm \
      -v "$PWD:/cennznet" \
      rust-builder:$NIGHTLY_DATE ./scripts/build-wasm.sh

# Create cennznet-node native binary
echo -e "\nBuilding cennznet node binary..."
docker run --user "$(id -u)":"$(id -g)" \
      -t --rm \
      -v "$PWD:/cennznet" \
      rust-builder:$NIGHTLY_DATE ./scripts/build-binary.sh

# Create a cennznet-node image
echo -e "\nBuilding cennznet node image..."
IMAGE_NAME="${IMAGE_NAME:-cennznet-node}"
docker build --pull -f docker/binary.Dockerfile -t "$IMAGE_NAME" .
