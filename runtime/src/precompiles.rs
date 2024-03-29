use crate::{
	constants::evm::{CENNZX_PRECOMPILE, FEE_PROXY, NFT_PRECOMPILE, PEG_PRECOMPILE},
	AddressMappingOf, CENNZnetGasWeightMapping, Cennzx, EthStateOracle, Origin, Runtime, StateOracleIsActive,
	StateOraclePrecompileAddress,
};
use cennznet_primitives::types::{AssetId, CollectionId, SeriesId};
use crml_support::{ContractExecutor, H160, U256};
use frame_support::{dispatch::DispatchResultWithPostInfo, traits::Get};
use pallet_evm::{Context, Precompile, PrecompileResult, PrecompileSet};
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
use pallet_evm_precompiles_nft::NftPrecompile;
use pallet_evm_precompiles_state_oracle::StateOraclePrecompile;
use precompile_utils::Gasometer;
use sp_std::{convert::TryInto, marker::PhantomData, prelude::*};

/// CENNZnet specific EVM precompiles
pub struct CENNZnetPrecompiles<R>(PhantomData<R>);

impl<R> CENNZnetPrecompiles<R>
where
	R: pallet_evm::Config,
{
	pub fn new() -> Self {
		Self(Default::default())
	}
	pub fn used_addresses() -> sp_std::vec::Vec<H160> {
		// TODO: precompute this
		sp_std::vec![
			1,
			2,
			3,
			4,
			5,
			9,
			1024,
			1026,
			CENNZX_PRECOMPILE,
			FEE_PROXY,
			PEG_PRECOMPILE,
			NFT_PRECOMPILE,
			27572 // TODO Create constant
		]
		.into_iter()
		.map(|x| hash(x))
		.collect()
	}
}
impl<R> PrecompileSet for CENNZnetPrecompiles<R>
where
	R: pallet_evm::Config,
{
	fn execute(
		&self,
		address: H160,
		input: &[u8],
		target_gas: Option<u64>,
		context: &Context,
		is_static: bool,
	) -> Option<PrecompileResult> {
		let routing_prefix = &address.to_fixed_bytes()[0..4];

		// Filter known precompile addresses except Ethereum officials
		if self.is_precompile(address) && address > hash(9) && address != context.address {
			let gasometer = Gasometer::new(target_gas);
			return Some(Err(gasometer.revert("cannot be called with DELEGATECALL or CALLCODE")));
		}

		match address {
			// Ethereum precompiles:
			a if a == hash(1) => Some(ECRecover::execute(input, target_gas, context, is_static)),
			a if a == hash(2) => Some(Sha256::execute(input, target_gas, context, is_static)),
			a if a == hash(3) => Some(Ripemd160::execute(input, target_gas, context, is_static)),
			a if a == hash(4) => Some(Identity::execute(input, target_gas, context, is_static)),
			a if a == hash(5) => Some(Modexp::execute(input, target_gas, context, is_static)),
			a if a == hash(9) => Some(Blake2F::execute(input, target_gas, context, is_static)),
			// Non-CENNZnet specific nor Ethereum precompiles:
			a if a == hash(1024) => Some(Sha3FIPS256::execute(input, target_gas, context, is_static)),
			a if a == hash(1026) => Some(ECRecoverPublicKey::execute(input, target_gas, context, is_static)),
			// CENNZnet precompiles:
			a if a == hash(FEE_PROXY) => None,
			a if a == StateOraclePrecompileAddress::get() && StateOracleIsActive::get() => Some(
				StateOraclePrecompile::<EthStateOracle, Runtime>::execute(input, target_gas, context, is_static),
			),
			a if a == hash(PEG_PRECOMPILE) => Some(Erc20PegPrecompile::<Runtime>::execute(
				input, target_gas, context, is_static,
			)),
			a if a == hash(NFT_PRECOMPILE) => {
				Some(NftPrecompile::<Runtime>::execute(input, target_gas, context, is_static))
			}
			a if a == hash(CENNZX_PRECOMPILE) => Some(CennzxPrecompile::<
				Cennzx,
				AddressMappingOf<Runtime>,
				CENNZnetGasWeightMapping,
				Runtime,
			>::execute(input, target_gas, context, is_static)),
			_a if routing_prefix == ERC721_PRECOMPILE_ADDRESS_PREFIX => {
				<Erc721PrecompileSet<Runtime> as PrecompileSet>::execute(
					&Erc721PrecompileSet::<Runtime>::new(),
					address,
					input,
					target_gas,
					context,
					is_static,
				)
			}
			_a if routing_prefix == ERC20_PRECOMPILE_ADDRESS_PREFIX => {
				<Erc20PrecompileSet<Runtime> as PrecompileSet>::execute(
					&Erc20PrecompileSet::<Runtime>::new(),
					address,
					input,
					target_gas,
					context,
					is_static,
				)
			}
			_ => None,
		}
	}

	fn is_precompile(&self, address: H160) -> bool {
		let routing_prefix = &address.to_fixed_bytes()[0..4];
		Self::used_addresses().contains(&address)
			|| routing_prefix == ERC20_PRECOMPILE_ADDRESS_PREFIX
			|| routing_prefix == ERC721_PRECOMPILE_ADDRESS_PREFIX
	}
}

fn hash(a: u64) -> H160 {
	H160::from_low_u64_be(a)
}

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
