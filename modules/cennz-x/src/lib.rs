//!
//! CENNZ-X
//!
#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate srml_support as support;

extern crate srml_timestamp as timestamp;

use rstd::prelude::*;
use generic_asset;
use runtime_io::twox_128;
use runtime_primitives::traits::{As, Hash};
use support::{StorageDoubleMap, StorageMap, StorageValue, dispatch::Result};
use system::ensure_signed;

// An alias for the system wide `AccountId` type
pub type AccountIdOf<T> = <T as system::Trait>::AccountId;

pub trait Trait: system::Trait + generic_asset::Trait + timestamp::Trait {
	// This type is used as a shim from `system::Trait::Hash` to `system::Trait::AccountId`
	type AccountId: From<<Self as system::Trait>::Hash> + Into<<Self as system::Trait>::AccountId>;
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event<T>() = default;

		pub fn add_liquidity(
			origin,
			asset_id: T::AssetId,
			min_liquidity: T::Balance,
			max_asset_amount: T::Balance,
			core_amount: T::Balance,
			expire: T::Moment
		) {
			let origin = ensure_signed(origin)?;
			Self::_add_liquidity(origin, asset_id, min_liquidity, max_asset_amount, core_amount, expire)?;
		}
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
pub(crate) struct LiquidityBalance<T>(rstd::marker::PhantomData<T>);

impl<T: Trait> StorageDoubleMap for LiquidityBalance<T> {
	const PREFIX: &'static [u8] = b"cennz-x-spot:liquidity";
	type Key1 = (T::AssetId, T::AssetId);
	// Delete the whole pool
	type Key2 = AccountIdOf<T>;
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
		/// The Core AssetId
		pub CoreAssetId get(core_asset_id) config(): T::AssetId;
		pub FeeRate get(fee_rate) config(): (u32, u32);
		// Total supply of exchange token in existence.
		// Key: `(asset id, core asset id)`
		pub TotalSupply get(total_supply): map (T::AssetId, T::AssetId) => T::Balance;
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
impl<T: Trait> Module<T>
{
	/// Generates an exchange address for the given asset pair
	pub fn generate_exchange_address(asset: T::AssetId, core_asset: T::AssetId) -> AccountIdOf<T> {
		let mut buf = Vec::new();
		buf.extend_from_slice(b"cennz-x-spot:");
		buf.extend_from_slice(&u64_to_bytes(As::as_(core_asset)));
		buf.extend_from_slice(&u64_to_bytes(As::as_(asset)));

		// Use shim `system::Trait::Hash` -> `Trait::AccountId` -> system::Trait::AccountId`
		<T as Trait>::AccountId::from(T::Hashing::hash(&buf[..])).into()
	}

	fn ensure_not_expired(expire: T::Moment) -> Result {
		let now = <timestamp::Module<T>>::get();
		if expire < now {
			return Err("cennzx request expired");
		}
		Ok(())
	}

	// Storage R/W
	fn get_total_supply(asset_id: T::AssetId) -> T::Balance {
		let core_asset_id = Self::core_asset_id();
		<TotalSupply<T>>::get((asset_id, core_asset_id))
//		Self::total_supply((asset_id, core_asset_id))
	}

	fn mint_total_supply(asset_id: &T::AssetId, increase: &T::Balance) {
		let core_asset_id = Self::core_asset_id();
		<TotalSupply<T>>::mutate(
			(*asset_id, core_asset_id),
			|balance| { *balance + *increase });
	}

	fn burn_total_supply(asset_id: &T::AssetId, decrease: &T::Balance) {
		let core_asset_id = Self::core_asset_id();
		<TotalSupply<T>>::mutate(
			(*asset_id, core_asset_id),
			|balance| { *balance - *decrease });
	}

	fn set_liquidity(core_asset_id: T::AssetId, asset_id: T::AssetId, who: &AccountIdOf<T>, balance: T::Balance) {
		<LiquidityBalance<T>>::insert(&(core_asset_id, asset_id), who, balance);
	}

	pub fn get_liquidity(core_asset_id: T::AssetId, asset_id: T::AssetId, who: &AccountIdOf<T>) -> T::Balance {
		<LiquidityBalance<T>>::get(&(core_asset_id, asset_id), who).unwrap_or_else(Default::default)
	}

	//
	// Manage Liquidity
	//

	/// Deposit core asset and trade asset at current ratio to mint exchange assets
	/// Returns amount of exchange assets minted.
	///
	/// `asset_id` - The trade asset ID
	/// `min_liquidity` - The minimum liquidity to add
	/// `asset_amount` - Amount of trade asset to add
	/// `core_amount` - Amount of core asset to add
	/// `expire` - Amount of core asset to add
	pub fn _add_liquidity(
		from_account: AccountIdOf<T>,
		asset_id: T::AssetId,
		min_liquidity: T::Balance,
		max_asset_amount: T::Balance,
		core_amount: T::Balance,
		expire: T::Moment,
	) -> rstd::result::Result<T::Balance, &'static str> {
		Self::ensure_not_expired(expire)?;
		let core_asset_id = Self::core_asset_id();
		let total_liquidity = Self::get_total_supply(asset_id.clone());
		let exchange_address = Self::generate_exchange_address(asset_id.clone(), core_asset_id);
		if total_liquidity > T::Balance::sa(0) {
			// shall i use total_balance instead? in which case the exchange address will have reserve balance?
			let trade_asset_reserve = <generic_asset::Module<T>>::free_balance(&asset_id, &exchange_address);
			let core_asset_reserve = <generic_asset::Module<T>>::free_balance(&core_asset_id, &exchange_address);
			let trade_asset_amount = core_amount.clone() * trade_asset_reserve / core_asset_reserve + T::Balance::sa(1);
			let liquidity_minted = core_amount.clone() * total_liquidity / core_asset_reserve;
			ensure!( liquidity_minted >= min_liquidity, "Minimum liquidity is required");
			ensure!( max_asset_amount >= trade_asset_amount, "Token liquidity check unsuccessful");

			<generic_asset::Module<T>>::make_transfer(&core_asset_id, &from_account, &exchange_address, core_amount.clone())?;
			<generic_asset::Module<T>>::make_transfer(&asset_id, &from_account, &exchange_address, trade_asset_amount.clone())?;

			Self::set_liquidity(core_asset_id, asset_id, &from_account,
								Self::get_liquidity(core_asset_id, asset_id.clone(), &from_account) + liquidity_minted);
			Self::mint_total_supply(&asset_id, &liquidity_minted);
			Self::deposit_event(RawEvent::AddLiquidity(from_account.clone(), core_amount, asset_id, trade_asset_amount));
			Ok(liquidity_minted)
		} else {
			<generic_asset::Module<T>>::make_transfer(&core_asset_id, &from_account, &exchange_address, core_amount)?;
			<generic_asset::Module<T>>::make_transfer(&asset_id, &from_account, &exchange_address, max_asset_amount)?;
			let trade_asset_amount = max_asset_amount;
			let initial_liquidity = core_amount;
			Self::set_liquidity(core_asset_id, asset_id, &from_account, initial_liquidity);
			Self::mint_total_supply(&asset_id, &initial_liquidity);
			Self::deposit_event(RawEvent::AddLiquidity(from_account.clone(), initial_liquidity, asset_id.clone(), trade_asset_amount));
			Ok(initial_liquidity.clone())
		}
	}

	/// Burn exchange assets to withdraw core asset and trade asset at current ratio
	///
	/// `asset_id` - The trade asset ID
	/// `asset_amount` - Amount of exchange asset to burn
	/// `min_asset_withdraw` - The minimum trade asset withdrawn
	/// `min_core_withdraw` -  The minimum core asset withdrawn
	pub fn remove_liquidity(
		asset_id: T::AssetId,
		asset_amount: T::Balance,
		min_asset_withdraw: T::Balance,
		min_core_withdraw: T::Balance,
		expire: T::Moment,
	) {}

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
		expire: T::Moment,
		fee_rate: u32,
	) {}

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
		recipient: AccountIdOf<T>,
		expire: T::Moment,
		fee_rate: u32,
	) {}

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
		expire: T::Moment,
		fee_rate: u32,
	) {}

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
		recipient: AccountIdOf<T>,
		expire: T::Moment,
		fee_rate: u32,
	) {}

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
		expire: T::Moment,
		fee_rate: u32,
	) {}

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
		recipient: AccountIdOf<T>,
		expire: T::Moment,
		fee_rate: u32,
	) {}

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
		expire: T::Moment,
		fee_rate: u32,
	) {}

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
		recipient: AccountIdOf<T>,
		expire: T::Moment,
		fee_rate: u32,
	) {}

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
		expire: T::Moment,
		fee_rate: u32,
	) {}

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
		recipient: AccountIdOf<T>,
		expire: T::Moment,
		fee_rate: u32,
	) {}

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
		expire: T::Moment,
		fee_rate: u32,
	) {}

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
		recipient: AccountIdOf<T>,
		expire: T::Moment,
		fee_rate: u32,
	) {}

	//
	// Get Prices
	//

	/// `asset_id` - Trade asset
	/// `amount_sold` - Amount of core sold
	/// Returns amount of asset that can be bought with the input core
	pub fn core_to_asset_input_price(
		asset_id: T::AssetId,
		amount_sold: T::Balance,
		fee_rate: u32,
	) -> T::Balance {
		T::Balance::sa(0)
	}

	/// `asset_id` - Trade asset
	/// `amount_bought`- Amount of trade assets bought
	/// Returns amount of core needed to buy output assets.
	pub fn core_to_asset_output_price(
		asset_id: T::AssetId,
		amount_bought: T::Balance,
		fee_rate: u32,
	) -> T::Balance {
		T::Balance::sa(0)
	}

	/// `asset_id` - Trade asset
	/// `amount_sold` - Amount of trade assets sold
	/// Returns amount of core that can be bought with input assets.
	pub fn asset_to_core_input_price(
		asset_id: T::AssetId,
		amount_sold: T::Balance,
		fee_rate: u32,
	) -> T::Balance {
		T::Balance::sa(0)
	}

	/// `asset_id` - Trade asset
	/// `amount_bought` - Amount of output core
	/// Returns amount of trade assets needed to buy output core.
	pub fn asset_to_core_output_price(
		asset_id: T::AssetId,
		amount_bought: T::Balance,
		fee_rate: u32,
	) -> T::Balance {
		T::Balance::sa(0)
	}
}

#[cfg(test)]
mod tests {
	extern crate consensus;

	// The testing primitives are very useful for avoiding having to work with signatures
	// or public keys. `u64` is used as the `AccountId` and no `Signature`s are required.
	use runtime_primitives::{
		BuildStorage,
		testing::{Digest, DigestItem, Header},
		traits::{BlakeTwo256, IdentityLookup},
	};
	use runtime_io::with_externalities;
	use substrate_primitives::{Blake2Hasher, H256, Ed25519AuthorityId};

	use super::*;

	impl_outer_origin! {
		pub enum Origin for Test {}
	}

	// For testing the module, we construct most of a mock runtime. This means
	// first constructing a configuration type (`Test`) which `impl`s each of the
	// configuration traits of modules we want to use.
	#[derive(Clone, Eq, PartialEq)]
	pub struct TestAura;

	impl timestamp::OnTimestampSet<u64> for TestAura {
		fn on_timestamp_set(moment: u64) {
			unimplemented!()
		}
	}


	#[derive(Clone, Eq, PartialEq)]
	pub struct Test;

	impl system::Trait for Test {
		type Origin = Origin;
		type Index = u64;
		type BlockNumber = u64;
		type Hash = H256;
		type Hashing = BlakeTwo256;
		type Digest = Digest;
		type AccountId = H256;
		type Lookup = IdentityLookup<H256>;
		type Header = Header;
		type Event = ();
		type Log = DigestItem;
	}

	impl timestamp::Trait for Test {
		type Moment = u64;
		type OnTimestampSet = ();
	}

	impl consensus::Trait for Test {
		type Log = DigestItem;
		type SessionKey = Ed25519AuthorityId;
		type InherentOfflineReport = ();
	}

	impl generic_asset::Trait for Test {
		type Balance = u128;
		type AssetId = u32;
		type Event = ();
	}

	impl Trait for Test {
		type AccountId = H256;
		type Event = ();
	}

	type CennzXSpot = Module<Test>;

	pub struct ExtBuilder {
		core_asset_id: u32,
		fee_rate: (u32, u32),
	}

	impl Default for ExtBuilder {
		fn default() -> Self {
			Self {
				core_asset_id: 0,
				fee_rate: (3, 1000),
			}
		}
	}

	impl ExtBuilder {
		pub fn build(self) -> runtime_io::TestExternalities<Blake2Hasher> {
			let mut t = system::GenesisConfig::<Test>::default()
				.build_storage()
				.unwrap()
				.0;
			t.extend(
				GenesisConfig::<Test> {
					core_asset_id: self.core_asset_id,
					fee_rate: self.fee_rate,
				}
					.build_storage()
					.unwrap()
					.0,
			);
			runtime_io::TestExternalities::new(t)
		}
	}

	fn new_test_ext() -> runtime_io::TestExternalities<Blake2Hasher> {
		system::GenesisConfig::<Test>::default()
			.build_storage()
			.unwrap()
			.0
			.into()
	}

	#[test]
	fn the_first_investor_can_add_liquidity() {
		with_externalities(&mut ExtBuilder::default().build(), || {
			let core_asset_id = <CoreAssetId<Test>>::get();
			let next_asset_id = <generic_asset::Module<Test>>::next_asset_id();
			{
//				<timestamp::Module<Test>>::set_timestamp(20);
				<generic_asset::Module<Test>>::set_free_balance(
					&0,
					&H256::from_low_u64_be(1),
					100,
				);
				<generic_asset::Module<Test>>::set_free_balance(
					&1,
					&H256::from_low_u64_be(1),
					100,
				);
			}
			assert_eq!(CennzXSpot::_add_liquidity(
				H256::from_low_u64_be(1),
				1, //asset_id: T::AssetId,
				2, // min_liquidity: T::Balance,
				15, //max_asset_amount: T::Balance,
				10, //core_amount: T::Balance,
				10,//expire: T::Moment
			), Ok(10));

			let pool_address = CennzXSpot::generate_exchange_address(1, 0);

			assert_eq!(<generic_asset::Module<Test>>::free_balance(&0, &pool_address), 10);
			assert_eq!(<generic_asset::Module<Test>>::free_balance(&1, &pool_address), 15);

			assert_eq!(CennzXSpot::get_liquidity(0, 1, &H256::from_low_u64_be(1)), 10);

		});
	}
}
