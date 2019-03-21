# Runtime version 15 (master)
- Substrate commit: xxxxx
- Breaking changes:
- New features:
- New notable Substreate changes:

# Runtime version 14 (Kauri)
- Substrate commit: https://github.com/paritytech/substrate/commit/c4ca27f7121f37172ae0d2d92280a947ca77edd8
- Breaking changes:
	- New naming conventions for runtime modules, instead of `cennznet-modules-x`, use `crml-x`, where `crml` stands for CENNZNet Runtime Module Library.
- New features:
	- New runtime module `rewards`, which accumulates transaction fees and mint block reward.
		- Accumulated fees and minted block reward got contributed to staking rewards.
		- Block reward could be set in genesis config, also via `set_block_reward` in rewards module.
	- GA V4 fully implemented
	- More CENNZX features
- New notable Substreate changes:
	- No upstream substrate changes merged

# Runtime version 13 (Rimu)
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
- New notable Substreate changes:
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
