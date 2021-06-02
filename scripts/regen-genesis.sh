#!/bin/sh

# Regenerate genesis JSON files

if [[ "$1" == "azalea" ]]; then
  echo "Regenerating $1 genesis files"
  cargo run -- build-spec --chain=azalea > genesis/azalea.json
  cargo run -- build-spec --chain=azalea --raw > genesis/azalea.raw.json
elif [[ "$1" == "nikau" ]]; then
  echo "Regenerating $1 genesis files"
  cargo run -- build-spec --chain=nikau > genesis/nikau.json
  cargo run -- build-spec --chain=nikau --raw > genesis/nikau.raw.json
elif [[ "$1" == "rata" ]]; then
  echo "Regenerating $1 genesis files"
  cargo run -- build-spec --chain=rata > genesis/rata.json
  cargo run -- build-spec --chain=rata --raw > genesis/rata.raw.json  
elif [[ "$1" == "dev" ]]; then
  cargo run -- build-spec --chain=dev > genesis/dev.json
  cargo run -- build-spec --chain=dev --raw > genesis/dev.raw.json
else
  echo "usage ./scripts/regen-genesis.sh [azalea|nikau|dev]"
fi
