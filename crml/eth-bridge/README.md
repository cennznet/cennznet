# CENNZnet-Eth bridge

# TODO:
- Retrieve full validator set
- Implement token create + mint protocol
- Replay protection and claims time window
- Pass Eth host + API config to the OCW from commandline flags
- Test!!!
- Develop Eth-JSON-RPC api proxy service
- (maybe): allow notarization vote to include a 'failure' reason rather than false e..g not enough re-orgs or differing amounts

## Structure
```bash
#  solidity contract
contracts/
interfaces/
libraries/
# rust crate
src/
```

## Setup
```bash
# install
yarn
# compile
yarn build
# test
yarn test
```

## Geth setup

```bash
./geth-up.sh
```

Example CENNZnet account for accepting deposits
```
subkey inspect //BridgeTest
Secret Key URI `//BridgeTest` is account:
  Secret seed:      0x98e231c854da2ff30765b6b547197c3455be59b31dabeb092e05fdb97ba90b96
  Public key (hex): 0xacd6118e217e552ba801f7aa8a934ea6a300a5b394e7c3f42cd9d6dd9a457c10
  Account ID:       0xacd6118e217e552ba801f7aa8a934ea6a300a5b394e7c3f42cd9d6dd9a457c10
  SS58 Address:     5FyKggXKhqAwJ2o9oBu8j3WHbCfPCz3uCuhTc4fTDgVniWNU
```

example deposit event
```json
[
    {
        // erc20 contract
        "address": "0x458E4CE1Ee5f8E393006c797aa4D8c490CD57e6D",
        "topics": [
            "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
            "0x00000000000000000000000072d1b5d3fc22d2be6e1076435a11fe9863d8aeb2",
            "0x00000000000000000000000035e752f4ea0645ef8793b37b5757573ede504c47"
        ],
        "data": "0x0000000000000000000000000000000000000000000000000000000000003039",
        "blockNumber": 21,
        "transactionHash": "0xb2e5dbebff2f44503b2514ce2254899180e4244942af68def94ba45dcfa7a84a",
        "transactionIndex": 0,
        "blockHash": "0xb1dc17eaea52ccb042ef3daf404c34ab9a21eacd8fa471573a8b3e760a25776f",
        "logIndex": 0,
        "removed": false,
        "id": "log_b0a63c48"
    },
    {
        // erc20 contract
        "address": "0x458E4CE1Ee5f8E393006c797aa4D8c490CD57e6D",
        "topics": [
            "0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925",
            "0x00000000000000000000000072d1b5d3fc22d2be6e1076435a11fe9863d8aeb2",
            "0x00000000000000000000000035e752f4ea0645ef8793b37b5757573ede504c47"
        ],
        "data": "0x000000000000000000000000000000000000000000000000000000000001b207",
        "blockNumber": 21,
        "transactionHash": "0xb2e5dbebff2f44503b2514ce2254899180e4244942af68def94ba45dcfa7a84a",
        "transactionIndex": 0,
        "blockHash": "0xb1dc17eaea52ccb042ef3daf404c34ab9a21eacd8fa471573a8b3e760a25776f",
        "logIndex": 1,
        "removed": false,
        "id": "log_f8ea2071"
    },
    {
        // bridge contract
        "address": "0x35e752f4Ea0645Ef8793B37B5757573EdE504c47",
        "topics": [
            "0x260e406acb5c2890984616f2069afabc0e70de193cd93377cbe69426ef5334c5",
            "0x00000000000000000000000072d1b5d3fc22d2be6e1076435a11fe9863d8aeb2"
        ],
        "data": "0x000000000000000000000000458e4ce1ee5f8e393006c797aa4d8c490cd57e6d0000000000000000000000000000000000000000000000000000000000003039acd6118e217e552ba801f7aa8a934ea6a300a5b394e7c3f42cd9d6dd9a457c1",
        "blockNumber": 21,
        "transactionHash": "0xb2e5dbebff2f44503b2514ce2254899180e4244942af68def94ba45dcfa7a84a",
        "transactionIndex": 0,
        "blockHash": "0xb1dc17eaea52ccb042ef3daf404c34ab9a21eacd8fa471573a8b3e760a25776f",
        "logIndex": 2,
        "removed": false,
        "id": "log_671685bd"
    }
]
```