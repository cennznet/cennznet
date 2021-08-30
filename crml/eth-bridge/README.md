# CENNZnet-Eth bridge
The CENNZnet ethereum bridge relies on validators witnessing events on the Ethereum blockchain.
Bridge applications submit event claims (eth tx hash + EVM event data) to the bridge.
Validators independently check the Ethereum blockchain for a matching tx and event and cast subsequent notarization votes.
After a threshold of notarizations for an event are reached, the bridge application is notified of the validity and is able to act accordingly to fulfil the claim e.g mint tokens.

# Steps to go live:
- Retrieve full validator set ✅
- Implement token create + mint protocol ✅
- Replay protection and claims time window  ✅
- Pass Eth host + API config to the OCW from command-line flags ✅ (could do some optimization here)
- Develop delayed activation device (seems to exist already, rotate session keys + count active sessions keys vs. total (need to track total session keys some how..))  ✅
- Treat ERC20 as generic message passing  ✅
- (maybe): allow notarization vote to include the 'failure' reason rather than false e..g not enough re-orgs or differing amounts  ✅

TODO:
- Develop withdrawal process
- Move Eth contracts to standalone repo
- Handle per network bridge config without recompile runtime e.g. genesis properties file
- Handle CENNZ deposit edge case (leave to governance)
- Test!!!
- Write up design docs

## Run Bridged Validator
```bash
./target/debug/cennznet \
    --dev \
    --tmp \
    --unsafe-ws-external \
    --unsafe-rpc-external \
    --rpc-cors=all \
    --eth-http=http://localhost:8545
```