# ERC20 Peg
Manages a Generic Asset - ERC20 peg using the CENNZnet ethereum bridge.

## CENNZnet types
add to UI/Api session
```json
  "EventClaimId": "u64",
  "EthAddress": "H160",
  "EthHash": "H256",
  "EventTypeId": "u32",
  "Erc20DepositEvent": {
    "tokenType": "EthAddress",
    "amount": "U256",
    "beneficiary": "Address"
  }
```

## Structure
```bash
#  solidity contract
contracts/
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

## Run
deploy bridge contract locally
```bash
# start local Eth node
npx hardhat node
# deploy contracts and issue a test deposit
yarn deploy
```

## Other
Example CENNZnet account for accepting ERC20 deposits
```
subkey inspect //BridgeTest
Secret Key URI `//BridgeTest` is account:
  Secret seed:      0x98e231c854da2ff30765b6b547197c3455be59b31dabeb092e05fdb97ba90b96
  Public key (hex): 0xacd6118e217e552ba801f7aa8a934ea6a300a5b394e7c3f42cd9d6dd9a457c10
  Account ID:       0xacd6118e217e552ba801f7aa8a934ea6a300a5b394e7c3f42cd9d6dd9a457c10
  SS58 Address:     5FyKggXKhqAwJ2o9oBu8j3WHbCfPCz3uCuhTc4fTDgVniWNU
```

example ERC20 deposit event
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

## Testing
ensure contract address in module matches deployed

-> @cennznet/api branch with latest metadata and types
yarn build:release

@cennznet/types: npm link
@cennznet/api: npm link && npm link @cennznet/types
cennznet/cli: npm link @cennznet/api

replace 'rxjs/operators' with 'rxjs/operators/index.js' in packages/api/build/

cennznet: 0x1215b4Ec8161b7959A115805bf980e57A085c3E5
yolo: 0xbe4d356d1C68E22aFeE70B4510ec8b31e389c759

## submit claim
```js
    let depositTxHash = "0x18d8416388a673afec2b0b8b56f2bd47d155b1cd0a4287414a823bfeb58a6b77";
    let claim = {
         address: "0x1215b4ec8161b7959a115805bf980e57a085c3e5",
         amount: "1234",
         beneficiary: "0xacd6118e217e552ba801f7aa8a934ea6a300a5b394e7c3f42cd9d6dd9a457c10"
    };
    let result = await api.tx.erc20Peg.depositClaim(depositTxHash, claim).signAndSend(toyKeyring.alice);
```

## query tx receipt
request
```bash
curl localhost:8545 \
    -X POST \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","method":"eth_getTransactionReceipt","params": ["0x185e85beb3296c7339954811cc682e3f992573ad3eecd37409e0ed763448d303"],"id":1}'
```

response
```json
{"jsonrpc":"2.0","id":1,"result":{"blockHash":"0xa97fa85e0f38526be39a29eb77c07ad9f18c315f8eb6ab7d44028581c1518ec1","blockNumber":"0x5","contractAddress":null,"cumulativeGasUsed":"0x1685c","effectiveGasPrice":"0x30cb962f","from":"0xec2c80a819ee8e42c624f6a5de930e8184c0801f","gasUsed":"0x1685c","logs":[{"address":"0x17c54edee4d6bccf2379daa328dcc0fbd9c6ce2b","topics":["0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef","0x000000000000000000000000ec2c80a819ee8e42c624f6a5de930e8184c0801f","0x00000000000000000000000087015d61b82a3808d9720a79573bf75deb8a1e90"],"data":"0x000000000000000000000000000000000000000000000000000000000000007b","blockNumber":"0x5","transactionHash":"0x185e85beb3296c7339954811cc682e3f992573ad3eecd37409e0ed763448d303","transactionIndex":"0x0","blockHash":"0xa97fa85e0f38526be39a29eb77c07ad9f18c315f8eb6ab7d44028581c1518ec1","logIndex":"0x0","removed":false},{"address":"0x17c54edee4d6bccf2379daa328dcc0fbd9c6ce2b","topics":["0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925","0x000000000000000000000000ec2c80a819ee8e42c624f6a5de930e8184c0801f","0x00000000000000000000000087015d61b82a3808d9720a79573bf75deb8a1e90"],"data":"0x000000000000000000000000000000000000000000000000000000000001e1c5","blockNumber":"0x5","transactionHash":"0x185e85beb3296c7339954811cc682e3f992573ad3eecd37409e0ed763448d303","transactionIndex":"0x0","blockHash":"0xa97fa85e0f38526be39a29eb77c07ad9f18c315f8eb6ab7d44028581c1518ec1","logIndex":"0x1","removed":false},{"address":"0x87015d61b82a3808d9720a79573bf75deb8a1e90","topics":["0x76bb911c362d5b1feb3058bc7dc9354703e4b6eb9c61cc845f73da880cf62f61","0x000000000000000000000000ec2c80a819ee8e42c624f6a5de930e8184c0801f"],"data":"0x00000000000000000000000017c54edee4d6bccf2379daa328dcc0fbd9c6ce2b000000000000000000000000000000000000000000000000000000000000007bacd6118e217e552ba801f7aa8a934ea6a300a5b394e7c3f42cd9d6dd9a457c10","blockNumber":"0x5","transactionHash":"0x185e85beb3296c7339954811cc682e3f992573ad3eecd37409e0ed763448d303","transactionIndex":"0x0","blockHash":"0xa97fa85e0f38526be39a29eb77c07ad9f18c315f8eb6ab7d44028581c1518ec1","logIndex":"0x2","removed":false}],"logsBloom":"0x00000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000010000000200200000000000000000000000008000000000000000000000000000000000000000000000000000000001000000000000000000000000010000000000010000000800000000000000000000000000002000000000000000000000000000040000000020000000000000000000010000000000000000000000000000000000000000000000002000000000000000000000000200000000000008000000004000000000010001000000000000000020000000000000000000000000000001000000000","status":"0x1","to":"0x87015d61b82a3808d9720a79573bf75deb8a1e90","transactionHash":"0x185e85beb3296c7339954811cc682e3f992573ad3eecd37409e0ed763448d303","transactionIndex":"0x0","type":"0x0"}}
```