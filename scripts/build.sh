#!/usr/bin/env bash

# This script builds the wasm, however this isn't actually used anywhere and is kept to stay consistent with upstream substrate
set -e

PROJECT_ROOT=`git rev-parse --show-toplevel`
SRCS=(
  "runtime/wasm"
)

export CARGO_INCREMENTAL=0

# Save current directory.
pushd .

cd $ROOT

for SRC in "${SRCS[@]}"
do
  echo "*** Building wasm binaries in $SRC"
  cd "$PROJECT_ROOT/$SRC"

  ./build.sh

  cd - >> /dev/null
done

# Restore initial directory.
popd
