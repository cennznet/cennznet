# ETHY setup (leg-work)
- Connect Ethy in service.rs
- Add ECDSA keypairs to session pallet(?) / genesis config?

## TODO:
1) Persist voting storage somehow so we don't need to resync on start up...this will change depending on if we have check-pointing, frequency etc.
- can we refer to on-chain commitment?
- can we request current votes from others?
2) We're dealing with async stuff so need some r/w locks to prevent races when updating state
3) The validators could automatically vote to close the bridge if they can't progress consensus after some time period
4) full nodes shouldn't need an Eth notifier, they will only observe votes and progress rounds
5) Check blocks have enough confirmations before voting (can handle this in the Eth notifier)

## commitment protocol
goal: create proof an eth block is considered canonical by CENNZnet validators
publishing all signatures on each hash is going to be too costly on storage
32 bytes + 64 bytes * validators * 6000 = ~6.4 MB per day

Required confirmations = 12
Eth blocks per day = 6500
Checkpoint frequency blocks = 50
Checkpoints per day = 6500 / 100 = 65

- after gaining consensus on a block number what next?
- naive: commit to chain
- less naive: 


## eth notifier service
- establish pub/sub to eth-rpc api
- maintain buffer of blocks
- stores canonical block number
- send block (block hash, block number, confirmation count) messages on new blocks
- allow requests for (block number)

## anti-stall protocol
Assume all nodes will fail and messages will be lost
Need best effort mechanism to prevent stalls
- rebroadcast known votes periodically if consensus isn't reached (don't flood the network)
- request/reply known round state
- persist round state locally

# start up protocol:
- advance state using the most recent onchain checkpoint
- request known round state
