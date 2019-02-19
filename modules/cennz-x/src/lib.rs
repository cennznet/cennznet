//!
//! CENNZ-X
//!
#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate srml_support as support;

use generic_asset;
use runtime_io::{blake2_256, twox_128};
use runtime_primitives::traits::As;
use substrate_primitives::H256;
use support::{rstd::prelude::*, StorageDoubleMap};

pub trait Trait: system::Trait + generic_asset::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event<T>() = default;
	}
}

decl_event!(
	pub enum Event<T>
	where
		<T as system::Trait>::AccountId,
		<T as generic_asset::Trait>::AssetId,
		<T as generic_asset::Trait>::Balance
	{
		// Provider, core asset amount, trade asset id, trade asset amount
		AddLiquidity(AccountId, Balance, AssetId, Balance),
		// Provider, core asset amount, trade asset id, trade asset amount
		RemoveLiquidity(AccountId, Balance, AssetId, Balance),
		// Buyer, trade asset sold, core asset bought
		CoreAssetPurchase(AccountId, Balance, Balance),
		// Buyer, core asset sold, trade asset bought
		TradeAssetPurchase(AccountId, Balance, Balance),
		// Trade asset id, core asset id
		NewPool(AssetId, AssetId),
	}
);

/// Asset balance of each user in each exchange pool.
/// Key: `(core asset id, trade asset id), account_id`
pub(crate) struct AssetBalance<T>(rstd::marker::PhantomData<T>);

impl<T: Trait> StorageDoubleMap for AssetBalance<T> {
	const PREFIX: &'static [u8] = b"cennz-x:asset";
	type Key1 = (T::AssetId, T::AssetId); // Delete the whole pool
	type Key2 = T::AccountId;
	type Value = T::Balance;

	fn derive_key1(key1_data: Vec<u8>) -> Vec<u8> {
		twox_128(&key1_data).to_vec()
	}

	fn derive_key2(key2_data: Vec<u8>) -> Vec<u8> {
		key2_data
	}
}

/// Core asset balance of each user in each exchange pool.
/// Key: `(core asset id, trade asset id), account_id`
pub(crate) struct CoreAssetBalance<T>(rstd::marker::PhantomData<T>);

impl<T: Trait> StorageDoubleMap for CoreAssetBalance<T> {
	const PREFIX: &'static [u8] = b"cennz-x:core";
	type Key1 = (T::AssetId, T::AssetId);
	type Key2 = T::AccountId;
	type Value = T::Balance;

	fn derive_key1(key1_data: Vec<u8>) -> Vec<u8> {
		twox_128(&key1_data).to_vec()
	}

	fn derive_key2(key2_data: Vec<u8>) -> Vec<u8> {
		key2_data
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as CennzX {
		// Total supply of exchange token in existence.
		// Key: `(asset id, core asset id)`
		pub TotalSupply get(total_supply): map(T::AssetId, T::AssetId) => T::Balance;
	}
}

/// Convert a `u64` into its byte array representation
fn u64_to_bytes(x: u64) -> [u8; 8] {
	let mut buf: [u8; 8] = [0u8; 8];
	for i in 0..8 {
		buf[7 - i] = ((x >> i * 8) & 0xFF) as u8;
	}

	buf
}

// The main implementation block for the module.
impl<T: Trait> Module<T> {

	/// Generates an exchange address for the given asset pair
	fn generate_exchange_address(asset1: T::AssetId, asset2: T::AssetId) -> H256 {
		let mut buf = Vec::new();
		buf.extend_from_slice(b"cennzx-account-id");
		buf.extend_from_slice(&u64_to_bytes(As::as_(asset1)));
		buf.extend_from_slice(&u64_to_bytes(As::as_(asset2)));

		H256::from_slice(&blake2_256(&buf[..]))
	}

	//
	// Manage Liquidity
	//

	/// Deposit core asset and trade asset at current ratio to mint exchange assets
	/// Returns amount of exchange assets minted.
	///
	/// `asset_id` - The trade asset ID
	/// `core_asset_id` - The core asset ID e.g. CENNZ or SYLO or any core asset
	/// `asset_amount` - Amount of trade asset to add
	/// `core_amount` - Amount of core asset to add
	/// `min_liquidity` - The minimum liquidity to add
	pub fn add_liquiditiy(
		asset_id: T::AssetId,
		core_asset_id: T::AssetId,
		asset_amount: T::Balance,
		core_amount: T::Balance,
		min_liquidity: T::Balance,
	) {
	}

	/// Burn exchange assets to withdraw core asset and trade asset at current ratio
	///
	/// `asset_id` - The trade asset ID
	/// `core_asset_id` - The core asset ID e.g. CENNZ or SYLO or any core asset
	/// `asset_amount` - Amount of exchange asset to burn
	/// `min_asset_withdraw` - The minimum trade asset withdrawn
	/// `min_core_withdraw` -  The minimum core asset withdrawn
	pub fn remove_liquidity(
		asset_id: T::AssetId,
		core_asset_id: T::AssetId,
		asset_amount: T::Balance,
		min_asset_withdraw: T::Balance,
		min_core_withdraw: T::Balance,
	) {
	}

	//
	// Trade core to other asset
	//

	/// Convert core asset to trade asset. User specifies
	/// exact input(core asset) and minimum output.
	///
	/// `asset_id` - Trade asset ID
	/// `amount_sold` - Exact amount of trade asset to be sold
	/// `min_amount_bought` - Minimum core assets bought
	/// `expire` - The block height before which this trade is valid
	pub fn core_to_asset_swap_input(
		asset_id: T::AssetId,
		amount_sold: T::Balance,
		min_amount_bought: T::Balance,
		expire: u32,
	) {
	}

	/// Convert core asset to trade asset and transfer the trade asset to recipient from system account.
	/// User specifies exact input (core asset) and minimum output.
	///
	/// `asset_id` - Trade asset ID
	/// `amount_sold` - Exact amount of trade asset to be sold
	/// `min_amount_bought` - Minimum core assets bought
	/// `recipient` - The address that receives the output asset
	/// `expire` - The block height before which this trade is valid
	pub fn core_to_asset_transfer_input(
		asset_id: T::AssetId,
		amount_sold: T::Balance,
		min_amount_bought: T::Balance,
		recipient: T::AccountId,
		expire: u32,
	) {
	}

	/// Convert core asset to trade asset. User specifies
	/// maximum input (core asset) and exact output.
	///
	/// `asset_id` - Trade asset ID
	/// `amount_bought` - Amount of core asset purchased
	/// `max_amount_sold` -  Maximum trade asset sold
	/// `expire` - The block height before which this trade is valid
	pub fn core_to_asset_swap_output(
		asset_id: T::AssetId,
		amount_bought: T::Balance,
		max_amount_sold: T::Balance,
		expire: u32,
	) {
	}

	/// Convert core asset to trade asset and transfer the trade asset to recipient
	/// from system account. User specifies maximum input (core asset) and exact output
	///
	/// `asset_id` - Trade asset ID
	/// `amount_bought` - Amount of core asset purchased
	/// `max_amount_sold` -  Maximum trade asset sold
	/// `recipient` - The address that receives the output asset
	/// `expire` - The block height before which this trade is valid
	pub fn core_to_asset_transfer_output(
		asset_id: T::AssetId,
		amount_bought: T::Balance,
		max_amount_sold: T::Balance,
		recipient: T::AccountId,
		expire: u32,
	) {
	}

	//
	// Trade asset with core asset
	//

	/// Convert trade asset to core asset. User specifies exact
	/// input (trade asset) and minimum output.
	///
	/// `asset_id` - Trade asset ID
	/// `amount_sold` - Exact amount of trade asset to be sold
	/// `min_amount_bought` - Minimum core assets bought
	/// `expire` - The block height before which this trade is valid
	pub fn asset_to_core_swap_input(
		asset_id: T::AssetId,
		amount_sold: T::Balance,
		min_amount_bought: T::Balance,
		expire: u32,
	) {
	}

	/// Convert trade asset to core asset and transfer the core asset to recipient from system account.
	/// User specifies exact input (core asset) and minimum output.
	///
	/// `asset_id` - Trade asset ID
	/// `amount_sold` - Exact amount of trade asset to be sold
	/// `min_amount_bought` - Minimum core assets bought
	/// `recipient` - The address that receives the output asset
	/// `expire` - The block height before which this trade is valid
	pub fn asset_to_core_transfer_input(
		asset_id: T::AssetId,
		amount_sold: T::Balance,
		min_amount_bought: T::Balance,
		recipient: T::AccountId,
		expire: u32,
	) {
	}

	/// Convert trade asset to core asset. User specifies maximum input (trade asset) and exact output
	///
	/// `asset_id` - Trade asset ID
	/// `amount_bought` - Amount of core asset purchased
	/// `max_amount_sold` -  Maximum trade asset sold
	/// `expire` - The block height before which this trade is valid
	pub fn asset_to_core_swap_output(
		asset_id: T::AssetId,
		amount_bought: T::Balance,
		max_amount_sold: T::Balance,
		expire: u32,
	) {
	}

	/// Convert core asset to trade asset and transfer the trade asset to recipient from system account.
	/// User specifies maximum input (core asset) and exact output.
	///
	/// `asset_id` - Trade asset ID
	/// `amount_bought` - Amount of core asset purchased
	/// `max_amount_sold` -  Maximum trade asset sold
	/// `recipient` - The address that receives the output asset
	/// `expire` - The block height before which this trade is valid
	pub fn asset_to_core_transfer_output(
		asset_id: T::AssetId,
		amount_bought: T::Balance,
		max_amount_sold: T::Balance,
		recipient: T::AccountId,
		expire: u32,
	) {
	}

	//
	// Trade non-core asset to non-core asset
	//

	/// Convert trade asset1 to trade asset2 via core asset. User specifies
	/// exact input and minimum output.
	///
	/// `asset_sold` - Trade asset1 ID
	/// `asset_bought` - asset2 ID
	/// `amount_sold` - Exact amount of trade asset to be sold
	/// `min_amount_bought` - Minimum trade asset2 purchased
	/// `min_core_bought` - Minimum core purchased as intermediary
	/// `expire` - The block height before which this trade is valid
	pub fn asset_to_asset_swap_input(
		asset_sold: T::AssetId,
		asset_bought: T::AssetId,
		amount_sold: T::Balance,
		min_amount_bought: T::Balance,
		min_core_bought: T::Balance,
		expire: u32,
	) {
	}

	/// Convert trade asset1 to trade asset2 via core asset and transfer the
	/// trade asset2 to recipient from system account.User specifies exact input
	/// and minimum output.
	///
	/// `asset_sold` - Trade asset1 ID
	/// `asset_bought` - asset2 ID
	/// `amount_sold` - Exact amount of trade asset to be sold
	/// `min_amount_bought` - Minimum trade asset2 purchased
	/// `min_core_bought` - Minimum core purchased as intermediary
	/// `recipient` - The address that receives the output asset
	/// `expire` - The block height before which this trade is valid
	pub fn asset_to_asset_transfer_input(
		asset_sold: T::AssetId,
		asset_bought: T::AssetId,
		amount_sold: T::Balance,
		min_amount_bought: T::Balance,
		min_core_bought: T::Balance,
		recipient: T::AccountId,
		expire: u32,
	) {
	}

	/// Convert trade asset1 to trade asset2 via core asset. User specifies maximum
	/// input and exact output.
	///
	/// `asset_sold` - Trade asset1 ID
	/// `asset_bought` - Asset2 ID
	/// `amount_bought` - Amount of trade asset2 bought
	/// `max_amount_sold` - Maximum trade asset1 sold
	/// `max_core_sold` - Maximum core asset purchased as intermediary
	/// `expire` - The block height before which this trade is valid
	pub fn asset_to_asset_swap_output(
		asset_sold: T::AssetId,
		asset_bought: T::AssetId,
		amount_bought: T::Balance,
		max_amount_sold: T::Balance,
		max_core_sold: T::Balance,
		expire: u32,
	) {
	}

	/// Convert trade asset1 to trade asset2 via core asset and transfer the trade asset2
	/// to recipient from system account.
	///
	/// User specifies maximum input and exact output
	/// `asset_sold` - Trade asset1
	/// `asset_bought` - Asset2
	/// `amount_bought` - Amount of trade asset2 bought
	/// `max_amount_sold` - Maximum trade asset1 sold
	/// `max_core_sold` - Maximum core asset purchased as intermediary
	/// `recipient` - The address that receives the output asset
	/// `expire` - The block height before which this trade is valid
	pub fn asset_to_asset_transfer_output(
		asset_sold: T::AssetId,
		asset_bought: T::AssetId,
		amount_bought: T::Balance,
		max_amount_sold: T::Balance,
		max_core_sold: T::Balance,
		recipient: T::AccountId,
		expire: u32,
	) {
	}

	//
	// Get Prices
	//

	/// `asset_id` - Trade asset
	/// `amount_sold` - Amount of core sold
	/// Returns amount of asset that can be bought with the input core
	pub fn core_to_asset_input_price(asset_id: T::AssetId, amount_sold: T::Balance) -> T::Balance {
		T::Balance::sa(0)
	}

	/// `asset_id` - Trade asset
	/// `amount_bought`- Amount of trade assets bought
	/// Returns amount of core needed to buy output assets.
	pub fn core_to_asset_output_price(
		asset_id: T::AssetId,
		amount_bought: T::Balance,
	) -> T::Balance {
		T::Balance::sa(0)
	}

	/// `asset_id` - Trade asset
	/// `amount_sold` - Amount of trade assets sold
	/// Returns amount of core that can be bought with input assets.
	pub fn asset_to_core_input_price(asset_id: T::AssetId, amount_sold: T::Balance) -> T::Balance {
		T::Balance::sa(0)
	}

	/// `asset_id` - Trade asset
	/// `amount_bought` - Amount of output core
	/// Returns amount of trade assets needed to buy output core.
	pub fn asset_to_core_output_price(
		asset_id: T::AssetId,
		amount_bought: T::Balance,
	) -> T::Balance {
		T::Balance::sa(0)
	}
}
