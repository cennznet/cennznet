# Ethy

Is a protocol for generating collaborative message proofs using CENNZnet validators.
Validators receive proof requests via metadata in blocks (i.e. from the runtime) and sign a witness.
This is broadcast on a dedicated libp2p protocol, once a configurable threshold of validators have signed and
broadcasted proofs, each participant may construct a local proof.

The proof is simply a list of signatures from all validators over the given event.
This could be advanced to use threshold singing scheme in the future.
The proof is portable and useful for submitting to an accompanying Ethereum contract.
