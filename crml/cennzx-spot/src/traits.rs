use pallet_generic_asset::Trait;
use sp_runtime::DispatchError;
use sp_std::result;

type Result = resulT::Result<T::Balance, DispatchError>;
type FeeRate = types::FeeRate<types::PerMillion>;

pub trait Exchange<T: Trait> {
	/// Convert asset1 to asset2. User specifies maximum
	/// input and exact output.
	///  origin
	/// `recipient` - Account to receive asset_bought, defaults to origin if None
	/// `asset_sold` - asset ID 1 to sell
	/// `asset_bought` - asset ID 2 to buy
	/// `buy_amount` - The amount of asset '2' to purchase
	/// `max_paying_amount` - Maximum trade asset '1' to pay
	/// When successful return the actual amount paid
	fn asset_swap_output(
		origin: T::Origin,
		recipient: Option<T::AccountId>,
		#[compact] asset_sold: T::AssetId,
		#[compact] asset_bought: T::AssetId,
		#[compact] buy_amount: T::Balance,
		#[compact] max_paying_amount: T::Balance,
	) -> Result;

	/// Convert asset1 to asset2
	/// Seller specifies exact input (asset 1) and minimum output (asset 2)
	/// `recipient` - Account to receive asset_bought, defaults to origin if None
	/// `asset_sold` - asset ID 1 to sell
	/// `asset_bought` - asset ID 2 to buy
	/// `sell_amount` - The amount of asset '1' to sell
	/// `min_receive` - Minimum trade asset '2' to receive from sale
	/// When successful return the amount of trade asset '2' that is received
	fn asset_swap_input(
		origin: T::Origin,
		recipient: Option<T::AccountId>,
		#[compact] asset_sold: T::AssetId,
		#[compact] asset_bought: T::AssetId,
		#[compact] sell_amount: T::Balance,
		#[compact] min_receive: T::Balance,
	) -> Result;
}

pub trait ManageLiquidity<T: Trait> {
	/// Deposit core asset and trade asset at current ratio to mint liquidity
	/// Returns amount of liquidity minted.
	///
	/// `origin`
	/// `asset_id` - The trade asset ID
	/// `min_liquidity` - The minimum liquidity to add
	/// `asset_amount` - Amount of trade asset to add
	/// `core_amount` - Amount of core asset to add
	/// When successful return the new level of liquidity
	fn add_liquidity(
		origin: T::Origin,
		#[compact] asset_id: T::AssetId,
		#[compact] min_liquidity: T::Balance,
		#[compact] max_asset_amount: T::Balance,
		#[compact] core_amount: T::Balance,
	) -> Result;

	/// Burn exchange assets to withdraw core asset and trade asset at current ratio
	///
	/// `asset_id` - The trade asset ID
	/// `asset_amount` - Amount of exchange asset to burn
	/// `min_asset_withdraw` - The minimum trade asset withdrawn
	/// `min_core_withdraw` -  The minimum core asset withdrawn
	/// When successful return the new level of liquidity
	fn remove_liquidity(
		origin: T::Origin,
		#[compact] asset_id: T::AssetId,
		#[compact] liquidity_withdrawn: T::Balance,
		#[compact] min_asset_withdraw: T::Balance,
		#[compact] min_core_withdraw: T::Balance,
	) -> Result;
}

pub trait ExchangePrice<T: Trait> {
	/// Set the spot exchange wide fee rate (root only)
	/// When successful, return the new fee rate
	fn set_fee_rate(origin: T::Origin, new_fee_rate: FeeRate) -> Result;

	/// `asset_id` - Trade asset
	/// `buy_amount`- Amount of the trade asset to buy
	/// Returns the amount of core asset needed to purchase `buy_amount` of trade asset.
	fn get_core_to_asset_output_price(asset_id: &T::AssetId, buy_amount: T::Balance, fee_rate: FeeRate) -> Result;

	/// `asset_id` - Trade asset
	/// `amount_sold` - Amount of the trade asset to sell
	/// Returns amount of core that can be bought with input assets.
	fn get_asset_to_core_input_price(asset_id: &T::AssetId, sell_amount: T::Balance, fee_rate: FeeRate) -> Result;

	/// `asset_id` - Trade asset
	/// `buy_amount` - Amount of output core
	/// `fee_rate` - The % of exchange fees for the trade
	/// Returns the amount of trade assets needed to buy `buy_amount` core assets.
	fn get_asset_to_core_output_price(asset_id: &T::AssetId, buy_amount: T::Balance, fee_rate: FeeRate) -> Result;

	/// `asset_id` - Trade asset
	/// `sell_amount` - Amount of input core to sell
	/// `fee_rate` - The % of exchange fees for the trade
	/// Returns the amount of trade asset to pay for `sell_amount` of core sold.
	fn get_core_to_asset_input_price(asset_id: &T::AssetId, sell_amount: T::Balance, fee_rate: FeeRate) -> Result;
}
