- update to 4.0.0-dev
- switch back to cennznet/substrate
- staking module rebuild

crate update progress
---
crml
- [] generic-asset
- [] governance
- [] nft
- [] transaction-payment
- [] eth-bridge
- [x] eth-wallet
- [x] erc20-peg
- [x] support
- [] staking
[] ethy-gadget
[] cli
[] runtime

## Migration Notes

- runtime deps updated to 4.0.0-dev
- cli deps updated to 0.10.0-dev
- scale-info import required on most crml crates
- scale-info derive `TypeInfo` needed on most types
- warning: <frame_system::Module<T>> -> <frame_system::Pallet<T>>