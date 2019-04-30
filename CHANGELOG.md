# Runtime version 20 (master)
- Substrate commit: xxx
- Breaking changes:
	- Removed `set_block_reward` in rewards module.
- New features:
	- Added `set_parameters` in rewards module, to calculate and set block reward and fee reward multiplier. 
- New notable Substrate changes:


# Runtime version 19
- Substrate commit: https://github.com/paritytech/substrate/commit/7d523bad168fe6867fdd6130e77ab749c38b9101
- Breaking changes:
	- Use ed25519 by default for network keys: https://github.com/paritytech/substrate/pull/2290
		- add `--node-key-type=secp256k1` to avoid breaking change
		- otherwise the libp2p node address will change
	- move storage maps to blake2_128: https://github.com/paritytech/substrate/pull/2268
		- Metadata V4
		- Needs update SDK version
- New features:
	- Start rework fees module
	- Update protocol id to avoid libp2p issues
- New notable Substrate changes:
	- Add `StorageValue::append` and speed-up `deposit_event`: https://github.com/paritytech/substrate/pull/2282

# Runtime version 18
- Substrate commit: https://github.com/paritytech/substrate/commit/7c6474663cdba40422760d21ae0119bfad425e40
- Breaking changes:
	- Update Substrate to v1.0rc
		- Use sr25519 instead of ed25519
		- Update address format checksum
	- Moved `attestation` and `fees` under new module named `prml` and they should be referred as `prml-attestation` and `prml-fees`.
- New features:
- New notable Substrate changes:
	- Many bug fixes in consensus and libp2p part

# Runtime version 17
- Substrate commit: https://github.com/paritytech/substrate/commit/c4ca27f7121f37172ae0d2d92280a947ca77edd8
- Breaking changes:
	- Updated Generic Asset Config for `dev.rs`, `testnet.rs` , `mainnet.rs` with test asset ids for CENNZ, CENTRAPAY, PLUG, SYLO, ARDA and CERTI
		- Test CENNZ is now `16000` and Test CENTRAPAY is now `16001`
- New features:
	- GA:
		- Transfer fee should be correctly applied now
		- Create new Asset now requires certain amount of CENNZ to be staked
	- Initial implementation of CennznetExtrinsic with Doughnut and CENNZX support (WIP)
- New notable Substrate changes:
	- No upstream substrate changes merged

# Runtime version 16
- No changes

# Runtime version 15
- Substrate commit: https://github.com/paritytech/substrate/commit/c4ca27f7121f37172ae0d2d92280a947ca77edd8
- Breaking changes:
	- Some types are renamed to avoid ambiguity
		- Sylo `vault::{Key, Val}` has been renamed to `VaultKey` and `VaultValue`
		- Attestation `Topic` and `Value` has been renamed to `AttestationTopic` and `AttestationValue`
	- New Sylo changes
	- New CENNXZ changes
- New notable Substrate changes:
	- No upstream substrate changes merged

# Runtime version 14
- Substrate commit: https://github.com/paritytech/substrate/commit/c4ca27f7121f37172ae0d2d92280a947ca77edd8
- Breaking changes:
	- New naming conventions for runtime modules, instead of `cennznet-modules-x`, use `crml-x`, where `crml` stands for CENNZNet Runtime Module Library.
- New features:
	- New runtime module `rewards`, which accumulates transaction fees and mint block reward.
		- Accumulated fees and minted block reward got contributed to staking rewards.
		- Block reward could be set in genesis config, also via `set_block_reward` in rewards module.
	- GA V4 fully implemented
	- More CENNZX features
- New notable Substrate changes:
	- No upstream substrate changes merged

# Runtime version 13
- Substrate commit: https://github.com/paritytech/substrate/commit/c4ca27f7121f37172ae0d2d92280a947ca77edd8
- Breaking changes:
	- GA
		- Type `AssetOptions` field `total_supply` renamed to `initial_issuance`
			- SDK type needs to be updated
		- Internal method `total_supply` renamed to `total_issuance`
	- balances module is been removed
		- use fees module to query transaction fee
		- use GA module for currency transfer
	- Staking: New Staking module
- New features:
	- New runtime module CENNZX
	- Sylo modules refactor
	- GA V4 (partially implemented, subject to change)
- New notable Substrate changes:
	- Add an RPC request for the state of the network https://github.com/paritytech/substrate/pull/1884
	- Stash/controller model for staking https://github.com/paritytech/substrate/pull/1782
	- Update parity-codec/-derive to 3.1 https://github.com/paritytech/substrate/pull/1900
		- Please update your branch to use parity-codec 3.1
	- Telemetry improvements https://github.com/paritytech/substrate/pull/1886\
	- Networking (p2p) improvements:
		- https://github.com/paritytech/substrate/pull/1934
		- https://github.com/paritytech/substrate/pull/1944
	- Extract specific transaction pool errors https://github.com/paritytech/substrate/pull/1930
	- Aggregate all liquidity restrictions in a single place https://github.com/paritytech/substrate/pull/1921
