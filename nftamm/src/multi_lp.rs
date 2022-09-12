use near_contract_standards::fungible_token::metadata::FungibleTokenMetadata;
use near_sdk::{
    assert_one_yocto, ext_contract, near_bindgen, Balance, PromiseOrValue, PromiseResult,
};

use crate::*;
use crate::{GAS_FOR_NFT_TRANSFER_CALL, GAS_FOR_RESOLVE_TRANSFER};

#[ext_contract(ext_self)]
trait MFTTokenResolver {
    fn lp_resolve_transfer(
        &mut self,
        pool_id: u64,
        sender_id: AccountId,
        receiver_id: AccountId,
        amount: U128,
    ) -> U128;
}

#[ext_contract(ext_lp_token_receiver)]
pub trait MFTTokenReceiver {
    fn lp_on_transfer(
        &mut self,
        pool_id: u64,
        sender_id: AccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128>;
}

#[near_bindgen]
impl Contract {
    fn internal_lp_transfer(
        &mut self,
        pool_id: u64,
        sender_id: &AccountId,
        receiver_id: &AccountId,
        amount: u128,
        memo: Option<String>,
    ) {
        // [AUDIT_07]
        require!(sender_id != receiver_id, "Cannot transfer to self");

        let pool = &mut self.pools[pool_id as usize];
        pool.lp_transfer(sender_id, receiver_id, amount);
        log!(
            "Transfer lp {} pool: {} from {} to {}",
            pool_id,
            amount,
            sender_id,
            receiver_id
        );

        if let Some(memo) = memo {
            log!("Memo: {}", memo);
        }
    }

    fn internal_lp_balance(&self, pool_id: u64, account_id: &AccountId) -> Balance {
        let pool = self.pools.get(pool_id as usize).expect("invalid pool_id");
        pool.lp_balances.get(account_id).unwrap_or(0)
    }

    /// Returns the balance of the given account. If the account doesn't exist will return `"0"`.
    pub fn lp_balance_of(&self, pool_id: u64, account_id: AccountId) -> U128 {
        self.internal_lp_balance(pool_id, &account_id).into()
    }

    /// Returns the total supply of the given token, if the token is one of the pools.
    /// If token references external token - fails with unimplemented.
    pub fn lp_total_supply(&self, pool_id: u64) -> U128 {
        let pool = self.pools.get(pool_id as usize).expect("invalid pool_id");
        pool.lp_supply.into()
    }

    /// Register LP token of given pool for given account.
    /// Fails if token_id is not a pool.
    #[payable]
    pub fn lp_register(&mut self, pool_id: u64, account_id: AccountId) {
        let prev_storage = env::storage_usage();
        let pool = &mut self.pools[pool_id as usize];
        pool.internal_register_account_lp(&account_id);
        let used_storage = env::storage_usage() - prev_storage;
        let used_near = used_storage as u128 * env::storage_byte_cost();
        require!(
            env::attached_deposit() >= used_near,
            "used near exceed atttached deposit"
        );
        Promise::new(env::predecessor_account_id()).transfer(env::attached_deposit() - used_near);
    }

    /// Transfer one of internal tokens: LP or balances.
    /// `token_id` can either by account of the token or pool number.
    #[payable]
    pub fn lp_transfer(
        &mut self,
        pool_id: u64,
        receiver_id: AccountId,
        amount: U128,
        memo: Option<String>,
    ) {
        assert_one_yocto();
        self.internal_lp_transfer(
            pool_id,
            &env::predecessor_account_id(),
            &receiver_id,
            amount.0,
            memo,
        );
    }

    #[payable]
    pub fn lp_transfer_call(
        &mut self,
        pool_id: u64,
        receiver_id: AccountId,
        amount: U128,
        memo: Option<String>,
        msg: String,
    ) -> PromiseOrValue<U128> {
        assert_one_yocto();
        let sender_id = env::predecessor_account_id();
        self.internal_lp_transfer(pool_id, &sender_id, &receiver_id, amount.0, memo);
        ext_lp_token_receiver::ext(receiver_id.clone())
            .with_static_gas(GAS_FOR_NFT_TRANSFER_CALL)
            .lp_on_transfer(pool_id, sender_id.clone(), amount, msg)
            .then(
                ext_self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_RESOLVE_TRANSFER)
                    .lp_resolve_transfer(pool_id, sender_id, receiver_id, amount),
            )
            .into()
    }

    /// Returns how much was refunded back to the sender.
    /// If sender removed account in the meantime, the tokens are sent to the contract account.
    /// Tokens are never burnt.
    #[private]
    pub fn lp_resolve_transfer(
        &mut self,
        pool_id: u64,
        sender_id: AccountId,
        receiver_id: &AccountId,
        amount: U128,
    ) -> U128 {
        let unused_amount = match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(value) => {
                if let Ok(unused_amount) = near_sdk::serde_json::from_slice::<U128>(&value) {
                    std::cmp::min(amount.0, unused_amount.0)
                } else {
                    amount.0
                }
            }
            PromiseResult::Failed => amount.0,
        };
        if unused_amount > 0 {
            let receiver_balance = self.internal_lp_balance(pool_id, &receiver_id);
            if receiver_balance > 0 {
                let refund_amount = std::cmp::min(receiver_balance, unused_amount);
                self.internal_lp_transfer(pool_id, &receiver_id, &sender_id, refund_amount, None);
            }
        }
        U128(unused_amount)
    }

    pub fn lp_metadata(&self, pool_id: u64) -> FungibleTokenMetadata {
        let decimals = 1u8;
        FungibleTokenMetadata {
            // [AUDIT_08]
            spec: "nearft-lp-1.0.0".to_string(),
            name: format!("nearft-pool-{}", pool_id),
            symbol: format!("NEARFT-POOL-{}", pool_id),
            icon: None,
            reference: None,
            reference_hash: None,
            decimals,
        }
    }
}
