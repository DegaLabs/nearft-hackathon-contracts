use near_sdk::{borsh::{self, BorshDeserialize, BorshSerialize}, PanicOnDefault, near_bindgen, serde::{Serialize, Deserialize}, env};
use super::{linear, exponential, BuyInfo, SellInfo};

#[near_bindgen]
#[repr(u8)]
#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub enum BondingCurve {
    LinearCurve = 0,
    ExponentialCurve = 1,
}

impl From<u8> for BondingCurve {
    fn from(val: u8) -> Self {
        match val {
            0u8 => BondingCurve::LinearCurve,
            1u8 => BondingCurve::ExponentialCurve,
            _ => env::panic_str("unknown bonding curve")
        }
    }
}

#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Curve {
    pub(crate) curve_type: BondingCurve
}

impl Curve {
    pub fn new(curve_type: BondingCurve) -> Curve {
        Curve {
            curve_type: curve_type
        }
    }

    pub(crate) fn validate_delta(&self, delta: u128) -> bool {
        match self.curve_type {
            BondingCurve::LinearCurve => linear::validate_delta(delta),
            BondingCurve::ExponentialCurve => exponential::validate_delta(delta)
        }
    }

    pub(crate) fn validate_spot_price(&self, new_spot_price: u128) -> bool {
        match self.curve_type {
            BondingCurve::LinearCurve => linear::validate_spot_price(new_spot_price),
            BondingCurve::ExponentialCurve => exponential::validate_spot_price(new_spot_price)
        }
    }

    pub(crate) fn get_buy_info(
        &self,
        spot_price: u128,
        delta: u128,
        num_items: u64,
        fee_multiplier: u128,
        protocol_fee_multiplier: u128,
    ) -> BuyInfo {
        match self.curve_type {
            BondingCurve::LinearCurve => linear::get_buy_info(spot_price, delta, num_items, fee_multiplier, protocol_fee_multiplier),
            BondingCurve::ExponentialCurve => exponential::get_buy_info(spot_price, delta, num_items, fee_multiplier, protocol_fee_multiplier)
        }
    }

    pub(crate) fn get_sell_info(
        &self, 
        spot_price: u128,
        delta: u128,
        num_items: u64,
        fee_multiplier: u128,
        protocol_fee_multiplier: u128,
    ) -> SellInfo {
        match self.curve_type {
            BondingCurve::LinearCurve => linear::get_sell_info(spot_price, delta, num_items, fee_multiplier, protocol_fee_multiplier),
            BondingCurve::ExponentialCurve => exponential::get_sell_info(spot_price, delta, num_items, fee_multiplier, protocol_fee_multiplier)
        }
    }
}