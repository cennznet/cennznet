//! CENNZ-X utility functions

/// Calculate the sale value of `asset_for_sale` in terms of `asset_to_receive`
fn calculate_sale_value(
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