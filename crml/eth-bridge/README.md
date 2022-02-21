# CENNZnet-Eth bridge
The CENNZnet ethereum bridge relies on validators witnessing events on the Ethereum blockchain.
Bridge applications submit event claims (eth tx hash + EVM event data) to the bridge.
Validators independently check the Ethereum blockchain for a matching tx and event and cast subsequent notarization votes.
After a threshold of notarizations for an event are reached, the bridge application is notified of the validity and is able to act accordingly to fulfil the claim e.g mint tokens.
