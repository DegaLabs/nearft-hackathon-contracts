use crate::curves::{errorcodes::CurveErrorCode, WAD, BuyInfo, SellInfo, U256};

pub(crate) fn validate_delta(_delta: u128) -> bool {
    //all valids for linear curve
    true
}

pub(crate) fn validate_spot_price(_new_spot_price: u128) -> bool {
    //all valids for linear curve
    true
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

    let new_spot_rice = spot_price + delta * (num_items as u128);
    if new_spot_rice > u128::MAX {
        return BuyInfo {
            error_code: CurveErrorCode::SpotPriceOverflow,
            new_spot_price: 0,
            new_delta: 0,
            input_value: U256::from(0),
            protocol_fee: U256::from(0),
        };
    }

    let buy_spot_price = spot_price + delta;
    let mut input_value = U256::from(num_items) * U256::from(buy_spot_price)
        + U256::from(num_items) * U256::from(num_items - 1) * U256::from(delta) / 2;
    let protocol_fee = (U256::from(input_value) * U256::from(protocol_fee_multiplier)) / WAD;

    input_value += (U256::from(input_value) * U256::from(fee_multiplier)) / WAD;
    input_value += protocol_fee;

    let new_delta = delta;

    BuyInfo {
        error_code: CurveErrorCode::Ok,
        new_spot_price: new_spot_rice,
        new_delta: new_delta,
        input_value: input_value,
        protocol_fee: protocol_fee,
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

    let total_price_decrease = U256::from(delta) * num_items;

    let mut new_spot_price = 0u128;
    let mut num_items = num_items;
    if U256::from(spot_price) < total_price_decrease {
        let num_items_till_zero_price = spot_price/delta + 1;
        num_items = num_items_till_zero_price as u64;
    } else {
        new_spot_price = spot_price - total_price_decrease.as_u128();
    }

    let mut output_value = U256::from(spot_price) * num_items - U256::from(num_items) * (num_items - 1) * U256::from(delta) / 2;
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
