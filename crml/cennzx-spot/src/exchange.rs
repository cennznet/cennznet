use crate::{traits::Exchange, Error, Module, Trait};

impl<T: Trait> Module<T> {
	/// Trade core asset for asset (`asset_id`) at the given `fee_rate`.
	/// `seller` - The address selling input asset
	/// `recipient` - The address receiving payment of output asset
	/// `asset_id` - The asset ID to trade
	/// `sell_amount` - Amount of core asset to sell (input)
	/// `min_receive` -  The minimum trade asset value to receive from sale (output)
	/// `fee_rate` - The % of exchange fees for the trade
	fn make_core_to_asset_input(
		seller: &T::AccountId,
		recipient: &T::AccountId,
		asset_id: &T::AssetId,
		sell_amount: T::Balance,
		min_receive: T::Balance,
		fee_rate: FeeRate<PerMillion>,
	) -> sp_std::result::Result<T::Balance, DispatchError> {
		let sale_value = Self::get_core_to_asset_input_price(asset_id, sell_amount, fee_rate)?;

		ensure!(sale_value > Zero::zero(), Error::<T>::AssetSaleValueNotAboveZero);
		ensure!(sale_value >= min_receive, Error::<T>::SaleValueBelowRequiredMinimum);
		let core_asset_id = Self::core_asset_id();
		ensure!(
			<pallet_generic_asset::Module<T>>::free_balance(&core_asset_id, seller) >= sell_amount,
			Error::<T>::InsufficientSellerCoreAssetBalance
		);

		let exchange_address = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, *asset_id);

		let _ =
			<pallet_generic_asset::Module<T>>::make_transfer(&core_asset_id, seller, &exchange_address, sell_amount)
				.and(<pallet_generic_asset::Module<T>>::make_transfer(
					asset_id,
					&exchange_address,
					recipient,
					sale_value,
				));

		Self::deposit_event(RawEvent::AssetPurchase(
			core_asset_id,
			*asset_id,
			seller.clone(),
			sell_amount,
			sale_value,
		));

		Ok(sale_value)
	}

	/// Trade asset (`asset_id`) to core asset at the given `fee_rate`
	/// `asset_id` - The asset ID to trade
	/// `buy_amount` - Amount of core asset to purchase (output)
	/// `max_paying_amount` -  Maximum asset to pay
	/// `fee_rate` - The % of exchange fees for the trade
	fn make_asset_to_core_output(
		buyer: &T::AccountId,
		recipient: &T::AccountId,
		asset_id: &T::AssetId,
		buy_amount: T::Balance,
		max_paying_amount: T::Balance,
		fee_rate: FeeRate<PerMillion>,
	) -> sp_std::result::Result<T::Balance, DispatchError> {
		let sold_amount = Self::get_asset_to_core_output_price(asset_id, buy_amount, fee_rate)?;
		ensure!(sold_amount > Zero::zero(), Error::<T>::AssetToCorePriceNotAboveZero);
		ensure!(
			max_paying_amount >= sold_amount,
			Error::<T>::AssetToCorePriceAboveMaxLimit
		);
		ensure!(
			<pallet_generic_asset::Module<T>>::free_balance(asset_id, buyer) >= sold_amount,
			Error::<T>::InsufficientBuyerTradeAssetBalance
		);

		let core_asset_id = Self::core_asset_id();
		let exchange_address = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, *asset_id);

		let _ = <pallet_generic_asset::Module<T>>::make_transfer(asset_id, buyer, &exchange_address, sold_amount).and(
			<pallet_generic_asset::Module<T>>::make_transfer(&core_asset_id, &exchange_address, recipient, buy_amount),
		);

		Self::deposit_event(RawEvent::AssetPurchase(
			*asset_id,
			core_asset_id,
			buyer.clone(),
			sold_amount,
			buy_amount,
		));

		Ok(sold_amount)
	}

	/// Trade core asset to asset (`asset_id`) at the given `fee_rate`
	/// `buyer` - Account buying core asset for trade asset
	/// `recipient` - Account receiving trade asset
	/// `asset_id` - The asset ID to trade
	/// `buy_amount` - Amount of core asset to purchase (output)
	/// `max_paying_amount` -  Maximum asset to pay
	/// `fee_rate` - The % of exchange fees for the trade
	fn make_core_to_asset_output(
		buyer: &T::AccountId,
		recipient: &T::AccountId,
		asset_id: &T::AssetId,
		buy_amount: T::Balance,
		max_paying_amount: T::Balance,
		fee_rate: FeeRate<PerMillion>,
	) -> sp_std::result::Result<T::Balance, DispatchError> {
		let sold_amount = Self::get_core_to_asset_output_price(asset_id, buy_amount, fee_rate)?;
		ensure!(sold_amount > Zero::zero(), Error::<T>::CoreToAssetPriceNotAboveZero);
		ensure!(
			max_paying_amount >= sold_amount,
			Error::<T>::CoreToAssetPriceAboveMaxLimit
		);
		let core_asset_id = Self::core_asset_id();
		ensure!(
			<pallet_generic_asset::Module<T>>::free_balance(&core_asset_id, buyer) >= sold_amount,
			Error::<T>::InsufficientBuyerCoreAssetBalance
		);

		let exchange_address = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, *asset_id);

		let _ = <pallet_generic_asset::Module<T>>::make_transfer(&core_asset_id, buyer, &exchange_address, sold_amount)
			.and(<pallet_generic_asset::Module<T>>::make_transfer(
				asset_id,
				&exchange_address,
				recipient,
				buy_amount,
			));

		Self::deposit_event(RawEvent::AssetPurchase(
			core_asset_id,
			*asset_id,
			buyer.clone(),
			sold_amount,
			buy_amount,
		));

		Ok(sold_amount)
	}

	/// Convert trade asset1 to trade asset2 via core asset. User specifies maximum
	/// input and exact output.
	/// `buyer` - Account buying core asset for trade asset
	/// `recipient` - Account receiving trade asset
	/// `asset_a` - asset ID to sell
	/// `asset_b` - asset ID to buy
	/// `buy_amount_b` - The amount of asset 'b' to purchase (output)
	/// `max_a_for_sale` - Maximum trade asset 'a' to sell
	/// `fee_rate` - The % of exchange fees for the trade
	fn make_asset_to_asset_output(
		buyer: &T::AccountId,
		recipient: &T::AccountId,
		asset_a: &T::AssetId,
		asset_b: &T::AssetId,
		buy_amount_for_b: T::Balance,
		max_a_for_sale: T::Balance,
		fee_rate: FeeRate<PerMillion>,
	) -> sp_std::result::Result<T::Balance, DispatchError> {
		// Calculate amount of core token needed to buy trade asset 2 of #buy_amount amount
		let core_for_b = Self::get_core_to_asset_output_price(asset_b, buy_amount_for_b, fee_rate)?;
		let core_asset_id = Self::core_asset_id();
		let exchange_address_a = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, *asset_a);
		let asset_sold_a = Self::get_asset_to_core_output_price(asset_a, core_for_b, fee_rate)?;
		// sold asset is always > 0
		ensure!(
			max_a_for_sale >= asset_sold_a,
			Error::<T>::AssetToAssetPriceAboveMaxLimit
		);
		ensure!(
			<pallet_generic_asset::Module<T>>::free_balance(&asset_a, buyer) >= asset_sold_a,
			Error::<T>::InsufficientBuyerTradeAssetBalance
		);

		let core_asset_a = Self::get_core_to_asset_output_price(asset_b, buy_amount_for_b, fee_rate)?;
		ensure!(core_asset_a > Zero::zero(), Error::<T>::CoreToAssetPriceNotAboveZero);
		ensure!(
			<pallet_generic_asset::Module<T>>::free_balance(&core_asset_id, &exchange_address_a) >= core_asset_a,
			Error::<T>::InsufficientCoreAssetInExchangeBalance
		);

		let exchange_address_b = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, *asset_b);
		let _ = <pallet_generic_asset::Module<T>>::make_transfer(&asset_a, buyer, &exchange_address_a, asset_sold_a)
			.and(<pallet_generic_asset::Module<T>>::make_transfer(
				&core_asset_id,
				&exchange_address_a,
				&exchange_address_b,
				core_asset_a,
			))
			.and(<pallet_generic_asset::Module<T>>::make_transfer(
				asset_b,
				&exchange_address_b,
				recipient,
				buy_amount_for_b,
			));

		Self::deposit_event(RawEvent::AssetPurchase(
			*asset_a,         // asset sold
			*asset_b,         // asset bought
			buyer.clone(),    // buyer
			asset_sold_a,     // sold amount
			buy_amount_for_b, // bought amount
		));

		Ok(asset_sold_a)
	}

	/// Convert trade asset to core asset. User specifies exact
	/// input (trade asset) and minimum output.
	///
	/// `asset_id` - Trade asset ID
	/// `sell_amount` - Exact amount of trade asset to be sold
	/// `min_receive` - Minimum amount of core asset to receive from sale
	fn make_asset_to_core_input(
		buyer: &T::AccountId,
		recipient: &T::AccountId,
		asset_id: &T::AssetId,
		sell_amount: T::Balance,
		min_receive: T::Balance,
		fee_rate: FeeRate<PerMillion>,
	) -> sp_std::result::Result<T::Balance, DispatchError> {
		ensure!(
			<pallet_generic_asset::Module<T>>::free_balance(asset_id, buyer) >= sell_amount,
			Error::<T>::InsufficientSellerTradeAssetBalance
		);

		let sale_value = Self::get_asset_to_core_input_price(asset_id, sell_amount, fee_rate)?;

		ensure!(sale_value >= min_receive, Error::<T>::SaleValueBelowRequiredMinimum);

		let core_asset_id = Self::core_asset_id();
		let exchange_address = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, *asset_id);

		ensure!(
			<pallet_generic_asset::Module<T>>::free_balance(&core_asset_id, &exchange_address) >= sale_value,
			Error::<T>::InsufficientCoreAssetInExchangeBalance
		);

		let _ = <pallet_generic_asset::Module<T>>::make_transfer(asset_id, buyer, &exchange_address, sell_amount).and(
			<pallet_generic_asset::Module<T>>::make_transfer(&core_asset_id, &exchange_address, recipient, sale_value),
		);

		Self::deposit_event(RawEvent::AssetPurchase(
			*asset_id,
			core_asset_id,
			buyer.clone(),
			sell_amount,
			sale_value,
		));

		Ok(sale_value)
	}

	/// Convert trade asset1 to trade asset2 via core asset.
	/// Seller specifies exact input (asset 1) and minimum output (trade asset and core asset)
	/// `recipient` - Receiver of asset_bought
	/// `asset_a` - asset ID to sell
	/// `asset_b` - asset ID to buy
	/// `sell_amount_for_a` - The amount of asset to sell
	/// `min_b_from_sale` - Minimum trade asset 'b' to receive from sale
	fn make_asset_to_asset_input(
		seller: &T::AccountId,
		recipient: &T::AccountId,
		asset_a: &T::AssetId,
		asset_b: &T::AssetId,
		sell_amount_for_a: T::Balance,
		min_b_from_sale: T::Balance,
		fee_rate: FeeRate<PerMillion>,
	) -> sp_std::result::Result<T::Balance, DispatchError> {
		ensure!(
			<pallet_generic_asset::Module<T>>::free_balance(&asset_a, seller) >= sell_amount_for_a,
			Error::<T>::InsufficientSellerTradeAssetBalance
		);

		let core_asset_id = Self::core_asset_id();
		let exchange_address_a = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, *asset_a);
		let sale_value_a = Self::get_asset_to_core_input_price(asset_a, sell_amount_for_a, fee_rate)?;
		let asset_b_received = Self::get_core_to_asset_input_price(asset_b, sale_value_a, fee_rate)?;

		ensure!(asset_b_received > Zero::zero(), Error::<T>::AssetSaleValueNotAboveZero);
		ensure!(
			asset_b_received >= min_b_from_sale,
			Error::<T>::InsufficientSellAssetForRequiredMinimumBuyAsset
		);
		ensure!(
			<pallet_generic_asset::Module<T>>::free_balance(&core_asset_id, &exchange_address_a) >= sale_value_a,
			Error::<T>::InsufficientCoreAssetInExchangeBalance
		);

		let exchange_address_b = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, *asset_b);

		let _ =
			<pallet_generic_asset::Module<T>>::make_transfer(&asset_a, seller, &exchange_address_a, sell_amount_for_a)
				.and(<pallet_generic_asset::Module<T>>::make_transfer(
					&core_asset_id,
					&exchange_address_a,
					&exchange_address_b,
					sale_value_a,
				))
				.and(<pallet_generic_asset::Module<T>>::make_transfer(
					asset_b,
					&exchange_address_b,
					recipient,
					asset_b_received,
				));

		Self::deposit_event(RawEvent::AssetPurchase(
			*asset_a,          // asset sold
			*asset_b,          // asset bought
			seller.clone(),    // buyer
			sell_amount_for_a, // sold amount
			asset_b_received,  // bought amount
		));

		Ok(asset_b_received)
	}

	fn get_output_price(
		output_amount: T::Balance,
		input_reserve: T::Balance,
		output_reserve: T::Balance,
		fee_rate: FeeRate<PerMillion>,
	) -> sp_std::result::Result<T::Balance, DispatchError> {
		if input_reserve.is_zero() || output_reserve.is_zero() {
			Err(Error::<T>::EmptyExchangePool)?;
		}

		// Special case, in theory price should progress towards infinity
		if output_amount >= output_reserve {
			return Ok(T::Balance::max_value());
		}

		let output_amount_hp = HighPrecisionUnsigned::from(T::BalanceToUnsignedInt::from(output_amount).into());
		let output_reserve_hp = HighPrecisionUnsigned::from(T::BalanceToUnsignedInt::from(output_reserve).into());
		let input_reserve_hp = HighPrecisionUnsigned::from(T::BalanceToUnsignedInt::from(input_reserve).into());
		let denominator_hp = output_reserve_hp - output_amount_hp;
		let price_hp = input_reserve_hp
			.saturating_mul(output_amount_hp)
			.checked_div(denominator_hp)
			.ok_or::<Error<T>>(Error::<T>::DivideByZero)?;

		let price_lp_result: Result<LowPrecisionUnsigned, &'static str> = LowPrecisionUnsigned::try_from(price_hp);
		if price_lp_result.is_err() {
			Err(Error::<T>::Overflow)?;
		}
		let price_lp = price_lp_result.unwrap();
		let price_plus_one = price_lp
			.checked_add(One::one())
			.ok_or::<Error<T>>(Error::<T>::Overflow)?;
		let fee_rate_plus_one = fee_rate
			.checked_add(FeeRate::<PerMillion>::one())
			.ok_or::<Error<T>>(Error::<T>::Overflow)?;
		let output = fee_rate_plus_one
			.checked_mul(price_plus_one.into())
			.ok_or::<Error<T>>(Error::<T>::Overflow)?;
		Ok(T::UnsignedIntToBalance::from(output.into()).into())
	}

	fn get_input_price(
		input_amount: T::Balance,
		input_reserve: T::Balance,
		output_reserve: T::Balance,
		fee_rate: FeeRate<PerMillion>,
	) -> sp_std::result::Result<T::Balance, DispatchError> {
		if input_reserve.is_zero() || output_reserve.is_zero() {
			Err(Error::<T>::EmptyExchangePool)?;
		}

		let div_rate: FeeRate<PerMillion> = fee_rate
			.checked_add(FeeRate::<PerMillion>::one())
			.ok_or::<Error<T>>(Error::<T>::Overflow)?;

		let input_amount_scaled = FeeRate::<PerMillion>::from(T::BalanceToUnsignedInt::from(input_amount).into())
			.checked_div(div_rate)
			.ok_or::<Error<T>>(Error::<T>::DivideByZero)?;

		let input_reserve_hp = HighPrecisionUnsigned::from(T::BalanceToUnsignedInt::from(input_reserve).into());
		let output_reserve_hp = HighPrecisionUnsigned::from(T::BalanceToUnsignedInt::from(output_reserve).into());
		let input_amount_scaled_hp = HighPrecisionUnsigned::from(LowPrecisionUnsigned::from(input_amount_scaled));
		let denominator_hp = input_amount_scaled_hp + input_reserve_hp;
		let price_hp = output_reserve_hp
			.saturating_mul(input_amount_scaled_hp)
			.checked_div(denominator_hp)
			.ok_or::<Error<T>>(Error::<T>::DivideByZero)?;

		let price_lp_result: Result<LowPrecisionUnsigned, &'static str> = LowPrecisionUnsigned::try_from(price_hp);
		if price_lp_result.is_err() {
			Err(Error::<T>::Overflow)?;
		}
		let price_lp = price_lp_result.unwrap();

		Ok(T::UnsignedIntToBalance::from(price_lp).into())
	}

	/// Convert asset1 to asset2. User specifies maximum
	/// input and exact output.
	///  `buyer` - Account buying asset
	/// `recipient` - Account to receive asset_bought, defaults to origin if None
	/// `asset_sold` - asset ID 1 to sell
	/// `asset_bought` - asset ID 2 to buy
	/// `buy_amount` - The amount of asset '2' to purchase
	/// `max_paying_amount` - Maximum trade asset '1' to pay
	/// `fee_rate` - The % of exchange fees for the trade
	fn make_asset_swap_output(
		buyer: &T::AccountId,
		recipient: &T::AccountId,
		asset_sold: &T::AssetId,
		asset_bought: &T::AssetId,
		buy_amount: T::Balance,
		max_paying_amount: T::Balance,
		fee_rate: FeeRate<PerMillion>,
	) -> sp_std::result::Result<T::Balance, DispatchError> {
		let core_asset = Self::core_asset_id();
		ensure!(asset_sold != asset_bought, Error::<T>::AssetCannotSwapForItself);
		let sold_amount = if *asset_sold == core_asset {
			Self::make_core_to_asset_output(buyer, recipient, asset_bought, buy_amount, max_paying_amount, fee_rate)?
		} else if *asset_bought == core_asset {
			Self::make_asset_to_core_output(buyer, recipient, asset_sold, buy_amount, max_paying_amount, fee_rate)?
		} else {
			Self::make_asset_to_asset_output(
				buyer,
				recipient,
				asset_sold,
				asset_bought,
				buy_amount,
				max_paying_amount,
				fee_rate,
			)?
		};

		Ok(sold_amount)
	}

	/// Convert asset1 to asset2
	/// Seller specifies exact input (asset 1) and minimum output (asset 2)
	/// `seller` - Account selling asset
	/// `recipient` - Account to receive asset_bought, defaults to origin if None
	/// `asset_sold` - asset ID 1 to sell
	/// `asset_bought` - asset ID 2 to buy
	/// `sell_amount` - The amount of asset '1' to sell
	/// `min_receive` - Minimum trade asset '2' to receive from sale
	/// `fee_rate` - The % of exchange fees for the trade
	fn make_asset_swap_input(
		seller: &T::AccountId,
		recipient: &T::AccountId,
		asset_sold: &T::AssetId,
		asset_bought: &T::AssetId,
		sell_amount: T::Balance,
		min_receive: T::Balance,
		fee_rate: FeeRate<PerMillion>,
	) -> Result<T> {
		let core_asset = Self::core_asset_id();
		ensure!(asset_sold != asset_bought, "Asset to swap should not be equal");
		if *asset_sold == core_asset {
			let _ =
				Self::make_core_to_asset_input(seller, recipient, asset_bought, sell_amount, min_receive, fee_rate)?;
		} else if *asset_bought == core_asset {
			let _ = Self::make_asset_to_core_input(seller, recipient, asset_sold, sell_amount, min_receive, fee_rate)?;
		} else {
			let _ = Self::make_asset_to_asset_input(
				seller,
				recipient,
				asset_sold,
				asset_bought,
				sell_amount,
				min_receive,
				fee_rate,
			)?;
		}

		Ok(())
	}
}

impl<T: Trait> Exchange<T> for Module<T> {
	fn asset_swap_output(
		origin: T::Origin,
		recipient: Option<T::AccountId>,
		#[compact] asset_sold: T::AssetId,
		#[compact] asset_bought: T::AssetId,
		#[compact] buy_amount: T::Balance,
		#[compact] max_paying_amount: T::Balance,
	) -> Result {
		let buyer = ensure_signed(origin)?;
		let amount_paid = Self::make_asset_swap_output(
			&buyer,
			&recipient.unwrap_or_else(|| buyer.clone()),
			&asset_sold,
			&asset_bought,
			buy_amount,
			max_paying_amount,
			Self::fee_rate(),
		)?;
		Ok(amount_paid)
	}

	fn asset_swap_input(
		origin: T::Origin,
		recipient: Option<T::AccountId>,
		#[compact] asset_sold: T::AssetId,
		#[compact] asset_bought: T::AssetId,
		#[compact] sell_amount: T::Balance,
		#[compact] min_receive: T::Balance,
	) -> Result {
		let seller = ensure_signed(origin)?;
		let amount_received = Self::make_asset_swap_input(
			&seller,
			&recipient.unwrap_or_else(|| seller.clone()),
			&asset_sold,
			&asset_bought,
			sell_amount,
			min_receive,
			Self::fee_rate(),
		)?;
		Ok(amount_received)
	}
}
