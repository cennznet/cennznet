use crate::{traits::ManageLiquidity, Error, Module, Trait};

type AssetId<T> = <T as pallet_generic_asset::Trait>::AssetId;

// TODO document
// TODO Make ExchangeKey hashable
// TODO Make ExchangeKey Encodeable
pub struct ExchangeKey<T: pallet_generic_asset::Trait> {
	core_asset: AssetId<T>,
	asset: AssetId<T>,
}

impl<T: Trait> Module<T> {
	fn get_total_supply(exchange_key: &ExchangeKey<T>) -> T::Balance {
		<TotalSupply<T>>::get(exchange_key)
	}

	/// mint total supply for an exchange pool
	fn mint_total_supply(exchange_key: &ExchangeKey<T>, increase: T::Balance) {
		<TotalSupply<T>>::mutate(exchange_key, |balance| *balance += increase); // will not overflow because it's limited by core assets's total supply
	}

	fn burn_total_supply(exchange_key: &ExchangeKey<T>, decrease: T::Balance) {
		<TotalSupply<T>>::mutate(exchange_key, |balance| *balance -= decrease); // will not underflow for the same reason
	}
}

impl<T: Trait> ManageLiquidity<T> for Module<T> {
	fn set_liquidity(#[compact] asset_id: T::AssetId, who: &T::AccountId, balance: T::Balance) {
		let key = crate::ExchangeKey::<T> {
			core_asset: Self::get_core_asset_id(),
			asset: asset_id,
		};
		<LiquidityBalance<T>>::insert(exchange_key, who, balance);
	}

	fn get_liquidity(#[compact] asset_id: T::AssetId, who: &T::AccountId) -> T::Balance {
		let key = crate::ExchangeKey::<T> {
			core_asset: Self::get_core_asset_id(),
			asset: asset_id,
		};
		<LiquidityBalance<T>>::get(key, who)
	}

	fn add_liquidity(
		origin: T::Origin,
		#[compact] asset_id: T::AssetId,
		#[compact] min_liquidity: T::Balance,
		#[compact] max_asset_amount: T::Balance,
		#[compact] core_amount: T::Balance,
	) -> Result {
		let from_account = ensure_signed(origin)?;
		let core_asset_id = Self::core_asset_id();
		ensure!(
			!max_asset_amount.is_zero(),
			Error::<T>::TradeAssetToAddLiquidityNotAboveZero
		);
		ensure!(!core_amount.is_zero(), Error::<T>::CoreAssetToAddLiquidityNotAboveZero);
		ensure!(
			<pallet_generic_asset::Module<T>>::free_balance(&core_asset_id, &from_account) >= core_amount,
			Error::<T>::CoreAssetBalanceToAddLiquidityTooLow
		);
		ensure!(
			<pallet_generic_asset::Module<T>>::free_balance(&asset_id, &from_account) >= max_asset_amount,
			Error::<T>::TradeAssetBalanceToAddLiquidityTooLow
		);
		let exchange_key = ExchangeKey::<T> {
			core_asset: core_asset_id,
			asset: asset_id,
		};
		let total_liquidity = Self::get_total_supply(&exchange_key);
		let exchange_address = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, asset_id);

		if total_liquidity.is_zero() {
			// new exchange pool
			<pallet_generic_asset::Module<T>>::make_transfer(
				&core_asset_id,
				&from_account,
				&exchange_address,
				core_amount,
			)?;
			<pallet_generic_asset::Module<T>>::make_transfer(
				&asset_id,
				&from_account,
				&exchange_address,
				max_asset_amount,
			)?;
			let trade_asset_amount = max_asset_amount;
			let initial_liquidity = core_amount;
			Self::set_liquidity(&exchange_key, &from_account, initial_liquidity);
			Self::mint_total_supply(&exchange_key, initial_liquidity);
			Self::deposit_event(RawEvent::AddLiquidity(
				from_account,
				initial_liquidity,
				asset_id,
				trade_asset_amount,
			));
		} else {
			// TODO: shall i use total_balance instead? in which case the exchange address will have reserve balance?
			let trade_asset_reserve = <pallet_generic_asset::Module<T>>::free_balance(&asset_id, &exchange_address);
			let core_asset_reserve = <pallet_generic_asset::Module<T>>::free_balance(&core_asset_id, &exchange_address);
			let trade_asset_amount = core_amount * trade_asset_reserve / core_asset_reserve + One::one();
			let liquidity_minted = core_amount * total_liquidity / core_asset_reserve;
			ensure!(
				liquidity_minted >= min_liquidity,
				Error::<T>::LiquidityMintableLowerThanRequired
			);
			ensure!(
				max_asset_amount >= trade_asset_amount,
				Error::<T>::TradeAssetToAddLiquidityAboveMaxAmount
			);

			<pallet_generic_asset::Module<T>>::make_transfer(
				&core_asset_id,
				&from_account,
				&exchange_address,
				core_amount,
			)?;
			<pallet_generic_asset::Module<T>>::make_transfer(
				&asset_id,
				&from_account,
				&exchange_address,
				trade_asset_amount,
			)?;

			Self::set_liquidity(
				&exchange_key,
				&from_account,
				Self::get_liquidity(&exchange_key, &from_account) + liquidity_minted,
			);
			Self::mint_total_supply(&exchange_key, liquidity_minted);
			Self::deposit_event(RawEvent::AddLiquidity(
				from_account,
				core_amount,
				asset_id,
				trade_asset_amount,
			));
		}

		Ok(Self::get_liquidity(&exchange_key, &from_account))
	}

	fn remove_liquidity(
		origin: T::Origin,
		#[compact] asset_id: T::AssetId,
		#[compact] liquidity_withdrawn: T::Balance,
		#[compact] min_asset_withdraw: T::Balance,
		#[compact] min_core_withdraw: T::Balance,
	) -> Result {
		let from_account = ensure_signed(origin)?;
		ensure!(
			liquidity_withdrawn > Zero::zero(),
			Error::<T>::LiquidityToWithdrawNotAboveZero
		);
		ensure!(
			min_asset_withdraw > Zero::zero() && min_core_withdraw > Zero::zero(),
			Error::<T>::AssetToWithdrawNotAboveZero
		);

		let core_asset_id = Self::core_asset_id();
		let exchange_key = (core_asset_id, asset_id);
		let account_liquidity = Self::get_liquidity(&exchange_key, &from_account);
		ensure!(account_liquidity >= liquidity_withdrawn, Error::<T>::LiquidityTooLow);

		let total_liquidity = Self::get_total_supply(&exchange_key);
		let exchange_address = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, asset_id);
		ensure!(total_liquidity > Zero::zero(), Error::<T>::NoLiquidityToRemove);

		let trade_asset_reserve = <pallet_generic_asset::Module<T>>::free_balance(&asset_id, &exchange_address);
		let core_asset_reserve = <pallet_generic_asset::Module<T>>::free_balance(&core_asset_id, &exchange_address);
		let core_asset_amount = liquidity_withdrawn * core_asset_reserve / total_liquidity;
		let trade_asset_amount = liquidity_withdrawn * trade_asset_reserve / total_liquidity;
		ensure!(
			core_asset_amount >= min_core_withdraw,
			Error::<T>::MinimumCoreAssetIsRequired
		);
		ensure!(
			trade_asset_amount >= min_asset_withdraw,
			Error::<T>::MinimumTradeAssetIsRequired
		);

		<pallet_generic_asset::Module<T>>::make_transfer(
			&core_asset_id,
			&exchange_address,
			&from_account,
			core_asset_amount,
		)?;
		<pallet_generic_asset::Module<T>>::make_transfer(
			&asset_id,
			&exchange_address,
			&from_account,
			trade_asset_amount,
		)?;
		Self::set_liquidity(&exchange_key, &from_account, account_liquidity - liquidity_withdrawn);
		Self::burn_total_supply(&exchange_key, liquidity_withdrawn);
		Self::deposit_event(RawEvent::RemoveLiquidity(
			from_account,
			core_asset_amount,
			asset_id,
			trade_asset_amount,
		));
		Ok(Self::get_liquidity(&exchange_key, &from_account))
	}
}
