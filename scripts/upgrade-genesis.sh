#!/usr/bin/env bash

echo Build WASM runtime
__dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
${__dir}/build.sh

if [ "$1" == "local" ]; then
	echo Updaing LOCAL genesis
	cargo run -- build-spec --chain=dev > ./genesis/local/readable.json
	cargo run -- build-spec --chain=./genesis/local/readable.json --raw > ./genesis/local/genesis.json
elif [ "$1" == "kauri" ]; then
	echo Updaing Kauri genesis
	cargo run -- build-spec --chain=kauri-latest > ./genesis/kauri/readable.json
	cargo run -- build-spec --chain=./genesis/kauri/readable.json --raw > ./genesis/kauri/genesis.json
elif [ "$1" == "rimu" ]; then
	echo Updaing Rimu genesis
	cargo run -- build-spec --chain=rimu-latest > ./genesis/rimu/readable.json
	cargo run -- build-spec --chain=./genesis/rimu/readable.json --raw > ./genesis/rimu/genesis.json
elif [ "$1" == "main" ]; then
	echo Updaing Mainnet genesis
	cargo run -- build-spec --chain=main-latest > ./genesis/main/readable.json
	cargo run -- build-spec --chain=./genesis/main/readable.json --raw > ./genesis/main/genesis.json
else
	echo "please provide chain name, valid values are: local, kauri, rimu, main"
    exit 1
fi
