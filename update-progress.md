- update to 4.0.0-dev
- switch back to cennznet/substrate
- staking module rebuild

crate update progress
---
crml
- [x] attestation (removed*)
- [x] generic-asset
    - [] tests
- [x] governance
    - [] tests
- [x] nft
    - [] tests
- [] transaction-payment
    - [] tests
- [x] eth-bridge
    - [] tests
- [x] eth-wallet
- [x] erc20-peg
- [x] support
- [x] sylo (removed*)
- [x] cennzx  
- [] staking
    - [] tests
[] ethy-gadget
    - [] tests
[] cli
    - [] tests
[] runtime
    - [] tests

*modules unused, removed to speed up update process

## Migration Notes

- runtime deps updated to 4.0.0-dev
- cli deps updated to 0.10.0-dev
- scale-info import required on most crml crates
- scale-info derive `TypeInfo` needed on most types
- warning: <frame_system::Module<T>> -> <frame_system::Pallet<T>>
- new prelude imports for pallets: `frame_system::pallet_prelude::*` && `frame_support:pallet_prelude::*`
- structs using T and deriving `TypeInfo` require `#[scale_info(skip_type_params(T))]`