# Ethy

Is a protocol for generating collaborative messages proofs using CENNZnet validators.
Validators receive proof requests via metadata in blocks (i.e. from the runtime) and sign a witness.
This is broadcast on a dedicated libp2p protocol, once a configurable threshold of validators have signed and
broadcasted proofs, each participant may construct a local proof.

The proof is simply a list of signatures from all validators over the given event.
This could be advanced to use threshold singing scheme in the future.
The proof is portable and useful for submitting to an accompanying Ethereum contract.

## TODO:
Essential:
- handle validator set force era
- Check the double spend 'bucketing' solution works
- Test we can query proofs from fullnodes ✅
- Check multiple claims in a block are supported  ✅
- Only update validator set proofs on a new era, not just session key changes  ✅
- Manage proofs in-flight on change? i.e pause proof requests for 1 session ~10 minutes ✅
- Distinguish validator set proof change (runtime log event w Id when proof requested)(?) ✅

QoL/Stability:x
- How to manage persistence on restart? (e.g. rebroadcast)
- Add rebroadcast behaviour
