//! Ethereum tx pool rpc runtime api & primitive types
use codec::{Decode, Encode};
pub use ethereum::{TransactionV0 as LegacyTransaction, TransactionV2 as Transaction};
use sp_runtime::{traits::Block as BlockT, RuntimeDebug};
use sp_std::vec::Vec;

/// Ethereum tx pool response (legacy)
#[derive(Eq, PartialEq, Clone, Encode, Decode, RuntimeDebug)]
pub struct TxPoolResponseLegacy {
	/// transactions in the ready state
	pub ready: Vec<LegacyTransaction>,
	/// transactions in the future state
	pub future: Vec<LegacyTransaction>,
}

/// Ethereum tx pool response
#[derive(Eq, PartialEq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "with-codec", derive(Encode, Decode))]
pub struct TxPoolResponse {
	/// transactions in the ready state
	pub ready: Vec<Transaction>,
	/// transactions in the future state
	pub future: Vec<Transaction>,
}

sp_api::decl_runtime_apis! {
	#[api_version(2)]
	pub trait TxPoolRuntimeApi {
		#[changed_in(2)]
		fn extrinsic_filter(
			xt_ready: Vec<<Block as BlockT>::Extrinsic>,
			xt_future: Vec<<Block as BlockT>::Extrinsic>,
		) -> TxPoolResponseLegacy;
		fn extrinsic_filter(
			xt_ready: Vec<<Block as BlockT>::Extrinsic>,
			xt_future: Vec<<Block as BlockT>::Extrinsic>,
		) -> TxPoolResponse;
	}
}
