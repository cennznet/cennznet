use crate::{traits::ExchangePrice, Error, Module, Trait};

impl<T: Trait> ExchangePrice<T> for Module<T> {
	fn set_fee_rate(origin: T::Origin, new_fee_rate: FeeRate<PerMillion>) -> Result {
		ensure_root(origin)?;
		DefaultFeeRate::mutate(|fee_rate| *fee_rate = new_fee_rate);
		Ok(DefaultFeeRate::get())
	}

	fn get_fee_rate() {
		DefaultFeeRate::get()
	}

	fn get_core_to_asset_output_price(asset_id: &T::AssetId, buy_amount: T::Balance) -> Result {
		ensure!(buy_amount > Zero::zero(), Error::<T>::BuyAmountNotPositive);

		let core_asset_id = Self::core_asset_id();
		let exchange_address = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, *asset_id);

		let asset_reserve = <pallet_generic_asset::Module<T>>::free_balance(asset_id, &exchange_address);
		ensure!(asset_reserve > buy_amount, Error::<T>::InsufficientTradeAssetReserve);

		let core_reserve = <pallet_generic_asset::Module<T>>::free_balance(&core_asset_id, &exchange_address);

		Self::get_output_price(buy_amount, core_reserve, asset_reserve, Self::fee_rate())
	}

	fn get_asset_to_core_input_price(asset_id: &T::AssetId, sell_amount: T::Balance) -> Result {
		ensure!(
			sell_amount > Zero::zero(),
			Error::<T>::AssetToCoreSellAmountNotAboveZero
		);

		let core_asset_id = Self::core_asset_id();
		let exchange_address = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, *asset_id);

		let asset_reserve = <pallet_generic_asset::Module<T>>::free_balance(asset_id, &exchange_address);
		let core_reserve = <pallet_generic_asset::Module<T>>::free_balance(&core_asset_id, &exchange_address);
		Self::get_input_price(sell_amount, asset_reserve, core_reserve, Self::fee_rate())
	}

	fn get_asset_to_core_output_price(asset_id: &T::AssetId, buy_amount: T::Balance) -> Result {
		ensure!(buy_amount > Zero::zero(), Error::<T>::BuyAmountNotPositive);

		let core_asset_id = Self::core_asset_id();
		let exchange_address = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, *asset_id);

		let core_asset_reserve = <pallet_generic_asset::Module<T>>::free_balance(&core_asset_id, &exchange_address);
		ensure!(
			core_asset_reserve > buy_amount,
			Error::<T>::InsufficientCoreAssetReserve
		);

		let trade_asset_reserve = <pallet_generic_asset::Module<T>>::free_balance(&asset_id, &exchange_address);

		Self::get_output_price(buy_amount, trade_asset_reserve, core_asset_reserve, Self::fee_rate())
	}

	fn get_core_to_asset_input_price(asset_id: &T::AssetId, sell_amount: T::Balance) -> Result {
		ensure!(
			sell_amount > Zero::zero(),
			Error::<T>::CoreToAssetSellAmountNotAboveZero
		);

		let core_asset_id = Self::core_asset_id();
		let exchange_address = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, *asset_id);
		let core_asset_reserve = <pallet_generic_asset::Module<T>>::free_balance(&core_asset_id, &exchange_address);
		let trade_asset_reserve = <pallet_generic_asset::Module<T>>::free_balance(asset_id, &exchange_address);

		let output_amount =
			Self::get_input_price(sell_amount, core_asset_reserve, trade_asset_reserve, Self::fee_rate())?;

		ensure!(
			trade_asset_reserve > output_amount,
			Error::<T>::InsufficientTradeAssetReserve
		);

		Ok(output_amount)
	}
}
