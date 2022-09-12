use crate::curves::{errorcodes::CurveErrorCode, WAD, BuyInfo, SellInfo, U256};
pub const MIN_PRICE: u128 = 10u128.pow(24);

fn fpow(x: U256, n: u64, base_unit: U256) -> U256 {
    let mut z = U256::zero();
    if x == U256::zero() {
        if n == 0 {
            z = base_unit;
        } else {
            //default z = 0   
        }
    } else {
        z = base_unit;
        for _i in 0..n {
            z = z * x / base_unit;
        }
    }
    z
}

pub(crate) fn validate_delta(delta: u128) -> bool {
    //all valids for linear curve
    delta > WAD
}

pub(crate) fn validate_spot_price(new_spot_price: u128) -> bool {
    //all valids for linear curve
    new_spot_price >= MIN_PRICE
}

pub(crate) fn get_buy_info(
    spot_price: u128,
    delta: u128,
    num_items: u64,
    fee_multiplier: u128,
    protocol_fee_multiplier: u128,
) -> BuyInfo {
    if num_items == 0 {
        return BuyInfo {
            error_code: CurveErrorCode::InvalidNumItem,
            new_spot_price: 0,
            new_delta: 0,
            input_value: U256::from(0),
            protocol_fee: U256::from(0),
        };
    }

    let delta_pow_n = U256::from(delta) * U256::from(num_items) / WAD;


    let new_spot_rice = U256::from(spot_price) * delta_pow_n / WAD;

    if new_spot_rice > U256::from(u128::MAX) {
        return BuyInfo {
            error_code: CurveErrorCode::SpotPriceOverflow,
            new_spot_price: 0,
            new_delta: 0,
            input_value: U256::from(0),
            protocol_fee: U256::from(0),
        };
    }

    let new_spot_rice = new_spot_rice.as_u128();

    let buy_spot_price = U256::from(spot_price) * U256::from(delta) / WAD;

    let mut input_value = buy_spot_price * ((delta_pow_n - WAD) * U256::from(WAD) / (delta - WAD)) / WAD;

    let protocol_fee = (U256::from(input_value) * U256::from(protocol_fee_multiplier)) / WAD;

    input_value += (U256::from(input_value) * U256::from(fee_multiplier)) / WAD;
    input_value += protocol_fee;

    let new_delta = delta;

    BuyInfo {
        error_code: CurveErrorCode::Ok,
        new_spot_price: new_spot_rice,
        new_delta: new_delta,
        input_value: input_value,
        protocol_fee: input_value,
    }
}

pub(crate) fn get_sell_info(
    spot_price: u128,
    delta: u128,
    num_items: u64,
    fee_multiplier: u128,
    protocol_fee_multiplier: u128,
) -> SellInfo {
    if num_items == 0 {
        return SellInfo {
            error_code: CurveErrorCode::InvalidNumItem,
            new_spot_price: 0,
            new_delta: 0,
            output_value: U256::from(0),
            protocol_fee: U256::from(0),
        };
    }

    let inv_delta = U256::from(WAD) * U256::from(WAD) / delta;
    let inv_delta_pow_n = fpow(inv_delta, num_items, U256::from(WAD));

    let new_spot_price = U256::from(spot_price) * inv_delta_pow_n / WAD;
    let mut new_spot_price = new_spot_price.as_u128();

    if new_spot_price < MIN_PRICE {
        new_spot_price = MIN_PRICE;
    }

    let mut output_value = U256::from(spot_price) * ((U256::from(WAD) - inv_delta_pow_n) * U256::from(WAD) / (U256::from(WAD) - inv_delta)) / WAD;

    let protocol_fee = output_value * U256::from(protocol_fee_multiplier) / WAD;

    output_value -= output_value * U256::from(fee_multiplier) / WAD;
    output_value -= protocol_fee;
    return SellInfo {
        error_code: CurveErrorCode::Ok,
        new_spot_price: new_spot_price,
        new_delta: delta,
        output_value: output_value,
        protocol_fee: protocol_fee,
    };
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::fpow;
    use crate::curves::{U256};

    #[test]
    fn test_fpow() {
        assert_eq!(fpow(4.into(), 8, 2.into()), U256::from(512u64));
    }
}
