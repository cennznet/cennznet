use crate::{constants::evm::*, AddressMappingOf, CENNZnetGasWeightMapping, Cennzx, EthStateOracle, Origin, Runtime};
use cennznet_primitives::types::{AssetId, CollectionId, SeriesId};
use crml_eth_state_oracle::STATE_ORACLE_PRECOMPILE;
use crml_support::{ContractExecutor, H160, U256};
use frame_support::{dispatch::DispatchResultWithPostInfo, parameter_types, traits::Get};
use pallet_evm_precompile_blake2::Blake2F;
use pallet_evm_precompile_modexp::Modexp;
use pallet_evm_precompile_sha3fips::Sha3FIPS256;
use pallet_evm_precompile_simple::{ECRecover, ECRecoverPublicKey, Identity, Ripemd160, Sha256};
use pallet_evm_precompiles_cennzx::CennzxPrecompile;
use pallet_evm_precompiles_erc20::{Erc20IdConversion, Erc20PrecompileSet, ERC20_PRECOMPILE_ADDRESS_PREFIX};
use pallet_evm_precompiles_erc20_peg::Erc20PegPrecompile;
use pallet_evm_precompiles_erc721::{
	Address, Erc721IdConversion, Erc721PrecompileSet, ERC721_PRECOMPILE_ADDRESS_PREFIX,
};
use pallet_evm_precompiles_state_oracle::StateOraclePrecompile;
use precompile_utils::precompile_set::*;
use sp_std::{convert::TryInto, marker::PhantomData, prelude::*};

parameter_types! {
	pub Erc721AssetPrefix: &'static [u8] = ERC721_PRECOMPILE_ADDRESS_PREFIX;
	pub Erc20AssetPrefix: &'static [u8] = ERC20_PRECOMPILE_ADDRESS_PREFIX;
}

/// The PrecompileSet installed in the CENNZnet runtime.
/// We include six of the nine Istanbul precompiles
/// (https://github.com/ethereum/go-ethereum/blob/3c46f557/core/vm/contracts.go#L69)
/// as well as a special precompile for dispatching Substrate extrinsics
/// The following distribution has been decided for the precompiles
/// 0-1023: Ethereum Mainnet Precompiles
pub type CENNZnetPrecompiles<R> = PrecompileSetBuilder<
	R,
	(
		// Skip precompiles if out of range.
		PrecompilesInRangeInclusive<
			(AddressU64<1>, AddressU64<4095>),
			(
				// Ethereum precompiles:
				// We allow DELEGATECALL to stay compliant with Ethereum behavior.
				PrecompileAt<AddressU64<1>, ECRecover, ForbidRecursion, AllowDelegateCall>,
				PrecompileAt<AddressU64<2>, Sha256, ForbidRecursion, AllowDelegateCall>,
				PrecompileAt<AddressU64<3>, Ripemd160, ForbidRecursion, AllowDelegateCall>,
				PrecompileAt<AddressU64<4>, Identity, ForbidRecursion, AllowDelegateCall>,
				PrecompileAt<AddressU64<5>, Modexp, ForbidRecursion, AllowDelegateCall>,
				PrecompileAt<AddressU64<9>, Blake2F, ForbidRecursion, AllowDelegateCall>,
				// Non-CENNZnet specific nor Ethereum precompiles :
				PrecompileAt<AddressU64<1024>, Sha3FIPS256>,
				PrecompileAt<AddressU64<1026>, ECRecoverPublicKey>,
				// CENNZnet specific precompiles:
				PrecompileAt<AddressU64<FEE_PROXY>, _>,
				PrecompileAt<AddressU64<STATE_ORACLE_PRECOMPILE>, StateOraclePrecompile<EthStateOracle, R>>,
				PrecompileAt<AddressU64<PEG_PRECOMPILE>, Erc20PegPrecompile<R>>,
				PrecompileAt<
					AddressU64<CENNZX_PRECOMPILE>,
					CennzxPrecompile<Cennzx, AddressMappingOf<R>, CENNZnetGasWeightMapping, R>,
				>,
			),
		>,
		// Prefixed precompile sets (XC20)
		PrecompileSetStartingWith<Erc721AssetPrefix, Erc721PrecompileSet<R>>,
		PrecompileSetStartingWith<Erc20AssetPrefix, Erc20PrecompileSet<R>>,
	),
>;

impl Erc721IdConversion for Runtime {
	type EvmId = Address;
	type RuntimeId = (CollectionId, SeriesId);

	// Get runtime Id from EVM address
	fn evm_id_to_runtime_id(evm_id: Self::EvmId) -> Option<Self::RuntimeId> {
		let h160_address: H160 = evm_id.into();
		let (prefix_part, id_part) = h160_address.as_fixed_bytes().split_at(4);

		if prefix_part == ERC721_PRECOMPILE_ADDRESS_PREFIX {
			let mut buf = [0u8; 16];
			buf.copy_from_slice(id_part);

			let collection_id = CollectionId::from_be_bytes(buf[0..4].try_into().ok()?);
			let series_id = SeriesId::from_be_bytes(buf[4..8].try_into().ok()?);

			Some((collection_id, series_id))
		} else {
			None
		}
	}
	// Get EVM address from series Id parts (collection_id, series_id)
	fn runtime_id_to_evm_id(series_id_parts: Self::RuntimeId) -> Self::EvmId {
		let mut buf = [0u8; 20];
		buf[0..4].copy_from_slice(ERC721_PRECOMPILE_ADDRESS_PREFIX);
		buf[4..8].copy_from_slice(&series_id_parts.0.to_be_bytes());
		buf[8..12].copy_from_slice(&series_id_parts.1.to_be_bytes());

		H160::from(buf).into()
	}
}

impl Erc20IdConversion for Runtime {
	type EvmId = Address;
	type RuntimeId = AssetId;

	// Get runtime Id from EVM address
	fn evm_id_to_runtime_id(evm_id: Self::EvmId) -> Option<Self::RuntimeId> {
		let h160_address: H160 = evm_id.into();
		let (prefix_part, id_part) = h160_address.as_fixed_bytes().split_at(4);

		if prefix_part == ERC20_PRECOMPILE_ADDRESS_PREFIX {
			let mut buf = [0u8; 4];
			buf.copy_from_slice(&id_part[..4]);
			let asset_id = AssetId::from_be_bytes(buf);

			Some(asset_id)
		} else {
			None
		}
	}
	// Get EVM address from series Id parts (collection_id, series_id)
	fn runtime_id_to_evm_id(asset_id: Self::RuntimeId) -> Self::EvmId {
		let mut buf = [0u8; 20];
		buf[0..4].copy_from_slice(ERC20_PRECOMPILE_ADDRESS_PREFIX);
		buf[4..8].copy_from_slice(&asset_id.to_be_bytes());

		H160::from(buf).into()
	}
}

/// Handles dispatching callbacks to the EVM after state oracle requests are fulfilled
pub struct StateOracleCallbackExecutor<R>(PhantomData<R>);

impl<R> ContractExecutor for StateOracleCallbackExecutor<R>
where
	R: pallet_ethereum::Config + pallet_evm::Config,
	R: frame_system::Config<Origin = Origin>,
{
	type Address = H160;
	/// Submit the state oracle callback transaction into the current block
	fn execute(
		caller: &Self::Address,
		target: &Self::Address,
		callback_input: &[u8],
		callback_gas_limit: u64,
		max_fee_per_gas: U256,
		max_priority_fee_per_gas: U256,
	) -> DispatchResultWithPostInfo {
		// must match the version used by `pallet_ethereum`
		use ethereum::{EIP1559Transaction, TransactionAction, TransactionV2};
		use pallet_ethereum::RawOrigin;

		let nonce = <pallet_evm::Pallet<R>>::account_basic(&caller).nonce;
		let callback_tx = TransactionV2::EIP1559(EIP1559Transaction {
			access_list: Default::default(),
			action: TransactionAction::Call(*target),
			chain_id: <R as pallet_evm::Config>::ChainId::get(),
			gas_limit: callback_gas_limit.into(),
			input: callback_input.to_vec(),
			max_fee_per_gas,
			max_priority_fee_per_gas,
			nonce,
			// the signature is inconsequential as this tx will be executed immediately, bypassing ordinary signature checks
			odd_y_parity: Default::default(),
			r: Default::default(),
			s: Default::default(),
			value: U256::zero(),
		});

		<pallet_ethereum::Pallet<R>>::transact(Origin::from(RawOrigin::EthereumTransaction(*caller)), callback_tx)
	}
}
