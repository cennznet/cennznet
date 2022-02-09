
## AddressMapping

- Convert Ethereum address to CENNZnet address
- Used by Precompiles to interact with runtime, should also be used by `crml-eth-wallet` for consistency
- It should be possible to get the original Ethereum address from the CENNZnet-ified version. This will enable smart contracts
to read Eth addresses from the runtime successfully.

In the case the runtime reads an Address which is not a native Ethereum address (normal CENNZnet) what should happen??
e.g. NFT minted via CENNZnet extrinsic

-> we could derive the ethereum address from the 32 byte public key
will this address still be valid if the user were to take that keypair and interact with a contract??

## Security
when using the prefix approach, the runtime accepts input from untrusted contracts.
We must be careful to ensure the mapping would not go to a valid 32 byte address?

Keen to hear your thoughts on this.

Our main goal is to mainly support Ethereum scripts/tooling out of the box.
We have a method to map Ethereum addresses to CENNZnet 32 byte addresses and its
reversible allowing future interaction between EVM smart contracts and the runtime modules.

We can derive an Ethereum address from a CENNZnet address using the standard approach ethereum takes from the public key.
But this is not reversible so we can't figure out 
 

mnemonic seed given to


everything has 32 byte address at this stage


# Option 1) 32 byte to EVM using normal method, stored mapping from EVM to cennznet address. User must 1 time login.

evm write to runtime:
must translate an ethereum address to its 32 byte equivalent
-> require one time user login tx. user signs a message that will register their EVM address against the CENNZnet 32 byte address `blake2(public_key)`
-> contracts will have an entry added to this mapping upon creation with some marker prefix bytes

evm read runtime:
take the 32 byte address and convert to 20 byte eth address using standard `keccak256[:-20]`

does this make valid address when 32 byte

---

# Option 2) Prefix mapping
evm write runtime:
- evm address pad to 32 bytes with marker prefix byte

evm read runtime:
if has marker prefix byte: 32 byte address unpad to 20 bytes
else: convert to eth address using normal algorithm

can the same address have 2 differnt balance entires?

will lead to two different balances of same token if user uses runtime or contract entrypoint