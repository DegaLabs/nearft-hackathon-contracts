pub const WAD: u128 = 10u128.pow(18);
use uint::construct_uint;
use crate::curves::errorcodes::CurveErrorCode;
construct_uint! {
    /// 256-bit unsigned integer.
    pub struct U256(4);
}
pub struct BuyInfo {
    pub error_code: CurveErrorCode,
    pub new_spot_price: u128,
    pub new_delta: u128,
    pub input_value: U256,
    pub protocol_fee: U256,
}

pub struct SellInfo {
    pub error_code: CurveErrorCode,
    pub new_spot_price: u128,
    pub new_delta: u128,
    pub output_value: U256,
    pub protocol_fee: U256,
}
mod linear;
pub mod errorcodes;
mod exponential;
pub mod curve;