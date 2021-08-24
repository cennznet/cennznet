# ETHY setup
- Make plug branch with ECDSA methods exposed
- Connect Ethy in service.rs


# Sketch
User burns funds (ERC20 peg module)
ERC20 Peg submits Eth event message to Bridge module
Bridge module adds a log event to the block
- ethy message notifier notifies about pre-message
- ethy message notifier notifies finality
ethy gadget observes event and
1) signs a witness
2) broadcast witness
3) collect witnesses
once a threshold of witnesses have been observed, store the witness in the db
broadcast the witness over RPC

# TODO:
- Receive withdraw signals from runtime after user initiates withdraw
- Receive validator set changes from runtime
https://github.com/paritytech/grandpa-bridge-gadget/blob/master/beefy-pallet/src/lib.rs
- Vote on withdraw/set change amongst peers
- Vote with 2/3rds or N append justification to a block
- RPC to allow querying withdrawal status / stored justification

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
