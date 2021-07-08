#!/bin/sh
echo "create test data dir..."
mkdir test-eth-data || true

echo "starting geth node.."
#docker rm -f cennznet-eth
docker run -p 8545:8545 -p 8546:8546 \
    --name=cennznet-eth \
    -v $(pwd)/test-eth-data:/test-eth-data \
    ethereum/client-go \
        --dev \
        --ipcpath "geth.ipc" \
        --datadir /test-eth-data \
        --http \
        --http.addr 0.0.0.0 \
        --http.api web3,eth,debug,personal,net \
        --http.corsdomain "https://remix.ethereum.org"
