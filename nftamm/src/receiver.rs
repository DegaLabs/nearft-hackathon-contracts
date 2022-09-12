use crate::*;
use near_contract_standards::non_fungible_token::core::NonFungibleTokenReceiver;
use near_sdk::{
    env,
    serde::{Deserialize, Serialize},
    PromiseOrValue, near_bindgen
};

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
#[serde(untagged)]
pub enum TokenReceiverMessage {
    /// Alternative to deposit + execute actions call.
    Deposit { pool_id: u32 },
}

#[near_bindgen]
impl NonFungibleTokenReceiver for Contract {
    fn nft_on_transfer(
        &mut self,
        sender_id: near_sdk::AccountId,
        previous_owner_id: near_sdk::AccountId,
        token_id: near_contract_standards::non_fungible_token::TokenId,
        msg: String,
    ) -> near_sdk::PromiseOrValue<bool> {
        let asset_id = env::predecessor_account_id();
        let account_id = previous_owner_id.clone();
        self.internal_deposit_nft_with_storage_check(&account_id, &asset_id, &token_id);
        PromiseOrValue::Value(true)
    }
}
