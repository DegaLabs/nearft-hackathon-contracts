use near_sdk::{borsh::{self, BorshDeserialize, BorshSerialize}, near_bindgen, serde::{Serialize, Deserialize}};

#[near_bindgen]
#[repr(u8)]
#[derive(BorshDeserialize, BorshSerialize, PartialEq, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[warn(non_camel_case_types)]
pub enum CurveErrorCode {
    Ok = 0,
    InvalidNumItem = 1,
    SpotPriceOverflow = 2
}