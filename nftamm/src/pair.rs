use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{UnorderedMap};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, require, AccountId, Balance, PanicOnDefault, near_bindgen};

use near_contract_standards::non_fungible_token::TokenId;

use crate::curves::curve::{BondingCurve, Curve};
use crate::curves::errorcodes::CurveErrorCode;
use crate::curves::{U256, WAD};
use crate::{AssetId, StorageKey};

pub const MAX_FEE: u128 = 9 * (10u128.pow(17)); //max 90%

#[near_bindgen]
#[repr(u8)]
#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub enum PoolType {
    Token = 0,
    NFT = 1,
    Trade = 2,
}

impl From<u8> for PoolType {
    fn from(val: u8) -> Self {
        match val {
            0u8 => PoolType::Token,
            1u8 => PoolType::NFT,
            2u8 => PoolType::Trade,
            _ => env::panic_str("unknown pool type")
        }
    }
}

#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct DepositedToken {
    depositor: AccountId,
    min_price: Balance,
}

// The spread between buy and sell prices, set to be a multiplier we apply to the buy price
// Fee is only relevant for TRADE pools
// Units are in base 1e18
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Pair {
    pub curve: Curve,
    pub pool_type: PoolType,
    pub nft_token: AssetId,
    pub spot_price: u128,
    pub delta: u128,
    pub fee: u128,
    pub owner: AccountId,
    // If set to none, NFTs/tokens sent by traders during trades will be sent to the pair.
    // Otherwise, assets will be sent to the set address. Not available for TRADE pools
    pub asset_recipient: Option<AccountId>,
    pub near_balance: Balance,
    pub token_ids_in_pools: UnorderedMap<TokenId, DepositedToken>,
    pub released_time: u64,
    pub pool_id: u64,
    pub lp_balances: UnorderedMap<AccountId, Balance>,
    pub lp_supply: Balance
}

impl Pair {
    pub fn new(
        curve_type: BondingCurve,
        pool_type: PoolType,
        nft_token: AssetId,
        spot_price: u128,
        delta: u128,
        fee: u128,
        owner: AccountId,
        asset_recipient: Option<AccountId>,
        initial_near_balance: Balance,
        released_time: u64,
        pool_id: u64
    ) -> Pair {
        let mut this = Pair {
            curve: Curve::new(curve_type),
            pool_type: pool_type,
            nft_token: nft_token,
            spot_price: spot_price,
            delta: delta,
            fee: fee,
            owner: owner,
            asset_recipient: None,
            near_balance: initial_near_balance,
            token_ids_in_pools: UnorderedMap::new(StorageKey::TokenIdsInPools {pool_id: pool_id}),
            released_time: released_time,
            pool_id: pool_id,
            lp_balances: UnorderedMap::new(StorageKey::PoolShare { pool_id: pool_id }),
            lp_supply: 0
        };

        if pool_type == PoolType::Token || pool_type == PoolType::NFT {
            require!(fee == 0u128, "only trade pools can have non zero fees");
            require!(
                asset_recipient.is_some(),
                "invalid asset recipient account id"
            );
            this.asset_recipient = asset_recipient;
        } else {
            require!(fee < MAX_FEE, "trade fee exceed max");
            require!(
                asset_recipient.is_none(),
                "asset recipient must be none for trade pools"
            );
        }

        require!(this.curve.validate_delta(this.delta), "invalid delta");
        require!(
            this.curve.validate_spot_price(this.spot_price),
            "Invalid new spot price for curve"
        );

        this
    }

    pub fn deposit_and_mint_lp(
        &mut self,
        depositor: AccountId,
        receiver_id: AccountId,
        token_ids: &Vec<TokenId>,
        near_balance: &Balance,
    ) {
        if self.pool_type == PoolType::Trade {
            require!(token_ids.len() as u128 * self.spot_price <= near_balance.clone(), "invalid added liquidity");
        }
        for token_id in token_ids {
            self.token_ids_in_pools.insert(
                token_id,
                &DepositedToken {
                    depositor: depositor.clone(),
                    min_price: 0u128,
                },
            );
        }
        self.near_balance += near_balance;

        // compute LP

        let mut lp_amount = self.near_balance;
        if self.lp_supply != 0 && !self.token_ids_in_pools.is_empty() {
            lp_amount = self.lp_supply * token_ids.len() as u128 / self.token_ids_in_pools.len() as u128;
        }
        self.mint_lp(&receiver_id, lp_amount);
    }

    fn internal_add_token_ids(
        &mut self,
        depositor: AccountId,
        token_ids: &Vec<TokenId>
    ) {
        for token_id in token_ids {
            self.token_ids_in_pools.insert(
                token_id,
                &DepositedToken {
                    depositor: depositor.clone(),
                    min_price: 0u128,
                },
            );
        }
    }

    pub fn withdraw_near(&mut self, near_amount: &Balance) -> Balance {
        require!(self.pool_type != PoolType::Trade, "not allowed to withdraw near directly from trading pool, need to burn lp token");
        self.assert_owner();
        self.assert_release();
        if self.near_balance > near_amount.clone() {
            self.near_balance -= near_amount;
            return near_amount.clone();
        }
        let ret = self.near_balance;
        self.near_balance = 0;
        ret
    }

    pub fn withdraw_nfts(&mut self, token_ids: &Vec<TokenId>) {
        require!(self.pool_type != PoolType::Trade, "not allowed to withdraw nfts directly from trading pool, need to burn lp token");
        self.assert_owner();
        self.assert_release();
        for token_id in token_ids {
            require!(
                self.token_ids_in_pools.get(token_id).is_some(),
                format!("token id {} not in pool", token_id)
            );
            self.token_ids_in_pools.remove(&token_id);
        }
    }

    pub fn swap_near_for_any_nfts(
        &mut self,
        deposit_near_amount: Balance,
        num_nfts: u64,
        protocol_fee_multiplier: u128,
    ) -> (Balance, Balance, Vec<TokenId>) {
        require!(
            self.pool_type == PoolType::NFT || self.pool_type == PoolType::Trade,
            "wrong pool type"
        );
        require!(
            num_nfts > 0 && num_nfts <= self.token_ids_in_pools.len(),
            "ask for > 0 or less than equal nfts in pool"
        );

        let (protocol_fee, input_amount) = self.calculate_buy_info_and_update_pool(
            num_nfts,
            deposit_near_amount,
            protocol_fee_multiplier,
        );
        let token_ids = self
            .token_ids_in_pools
            .keys()
            .take(num_nfts as usize)
            .collect::<Vec<TokenId>>();
        for token_id in &token_ids {
            self.token_ids_in_pools.remove(token_id);
        }
        let protocol_fee = protocol_fee.as_u128();
        if self.asset_recipient.is_none() {
            //trade pool, add the near input to the pool balance
            self.near_balance += input_amount - protocol_fee;
        } 

        (protocol_fee, input_amount, token_ids)
    }

    pub fn swap_near_for_specific_nfts(
        &mut self,
        deposit_near_amount: Balance,
        nft_ids: &Vec<TokenId>,
        protocol_fee_multiplier: u128,
    ) -> (Balance, Balance) {
        require!(
            self.pool_type == PoolType::NFT || self.pool_type == PoolType::Trade,
            "wrong pool type"
        );
        require!(nft_ids.len() > 0, "must ask for > 0 nfts");

        let (protocol_fee, input_amount) = self.calculate_buy_info_and_update_pool(
            nft_ids.len() as u64,
            deposit_near_amount,
            protocol_fee_multiplier,
        );

        for token_id in nft_ids {
            require!(
                self.token_ids_in_pools.get(token_id).is_some(),
                format!("token id {} not in pool", token_id)
            );
            self.token_ids_in_pools.remove(&token_id);
        }

        let protocol_fee = protocol_fee.as_u128();
        if self.asset_recipient.is_none() {
            //trade pool, add the near input to the pool balance
            self.near_balance += input_amount - protocol_fee;
        } 

        (protocol_fee, input_amount)
    }

    pub fn swap_nfts_for_near(
        &mut self,
        nft_ids: &Vec<TokenId>,
        min_near_out: Balance,
        protocol_fee_multiplier: u128,
    ) -> (Balance, Balance) {
        require!(
            self.pool_type == PoolType::Token || self.pool_type == PoolType::Trade,
            "wrong pool type"
        );
        require!(nft_ids.len() > 0, "ask for > 0");

        let (protocol_fee, mut output_amount) = self.calculate_sell_info_and_update_pool(
            nft_ids.len() as u64,
            min_near_out,
            protocol_fee_multiplier,
        );

        if self.near_balance >= output_amount {
            self.near_balance -= output_amount;
        } else {
            output_amount = self.near_balance;
            self.near_balance = 0;
        }
        let mut protocol_fee = protocol_fee.as_u128();
        if self.near_balance >= protocol_fee {
            self.near_balance -= protocol_fee;
        } else {
            protocol_fee = self.near_balance;
            self.near_balance = 0;
        }

        if self.asset_recipient.is_none() {
            //trading
            self.internal_add_token_ids(env::predecessor_account_id(), nft_ids);
        }

        require!(output_amount >= min_near_out, "insufficient liquidity");
        (protocol_fee, output_amount)
    }

    fn calculate_buy_info_and_update_pool(
        &mut self,
        num_nfts: u64,
        max_expected_near_input: Balance,
        protocol_fee_multiplier: u128,
    ) -> (U256, Balance) {
        let current_spot_price = self.spot_price;
        let current_delta = self.delta;
        let buy_info = self.curve.get_buy_info(
            current_spot_price,
            current_delta,
            num_nfts,
            self.fee,
            protocol_fee_multiplier,
        );
        if buy_info.error_code != CurveErrorCode::Ok {
            env::panic_str("failed to get buy info");
        }
        require!(
            buy_info.input_value <= U256::from(max_expected_near_input),
            "not enough near payment"
        );

        if current_spot_price != buy_info.new_spot_price || current_delta != buy_info.new_delta {
            self.spot_price = buy_info.new_spot_price;
            self.delta = buy_info.new_delta;
        }

        (buy_info.protocol_fee, buy_info.input_value.as_u128())
    }

    fn calculate_sell_info_and_update_pool(
        &mut self,
        num_nfts: u64,
        min_expected_near_output: Balance,
        protocol_fee_multiplier: u128,
    ) -> (U256, Balance) {
        let current_spot_price = self.spot_price;
        let current_delta = self.delta;

        let sell_info = self.curve.get_sell_info(
            current_spot_price,
            current_delta,
            num_nfts,
            self.fee,
            protocol_fee_multiplier,
        );
        if sell_info.error_code != CurveErrorCode::Ok {
            env::panic_str("failed to get sell info");
        }

        require!(
            sell_info.output_value.as_u128() >= min_expected_near_output,
            "out too little near"
        );

        if current_spot_price != sell_info.new_spot_price || current_delta != sell_info.new_delta {
            self.spot_price = sell_info.new_spot_price;
            self.delta = sell_info.new_delta;
        }

        (sell_info.protocol_fee, sell_info.output_value.as_u128())
    }

    pub fn lp_transfer(&mut self, sender_id: &AccountId, receiver_id: &AccountId, amount: u128) {
        let balance = self.lp_balances.get(&sender_id).expect("sender account not registered");
        if let Some(new_balance) = balance.checked_sub(amount) {
            self.lp_balances.insert(&sender_id, &new_balance);
        } else {
            env::panic_str("insufficient lp balance");
        }
        let balance_out = self
            .lp_balances
            .get(&receiver_id)
            .expect("receiver account not registered");
        self.lp_balances.insert(&receiver_id, &(balance_out + amount));
    }

    fn mint_lp(&mut self, account_id: &AccountId, lp: Balance) {
        if lp == 0 {
            return;
        }
        self.lp_supply += lp;
        let prev_value = self.lp_balances.get(account_id).unwrap_or(0);
        self.lp_balances.insert(account_id, &(prev_value + lp));
    }

    // the idea is if the amount of NFTs to withdraw is fraction, we round up the amount of NFT, and reduce the 
    // corresponding value of the fraction portion in the amount in near to withdraw
    pub fn burn_lp(&mut self, account_id: &AccountId, lp: Balance, protocol_fee_multiplier: u128) -> (Balance, Balance, Vec<TokenId>) {
        if lp == 0u128 {
            return (0, 0, vec![]);
        }
        
        let prev_value = self.lp_balances.get(account_id).unwrap_or(0);
        if lp > prev_value {
            env::panic_str("insufficient lp");
        }

        // compute withdrawnable nfts and liquidity
        let withdrawable_near = U256::from(self.token_ids_in_pools.len()) * U256::from(self.spot_price) * U256::from(lp) / U256::from(self.lp_supply);
        let mut withdrawable_near = withdrawable_near.as_u128();
        let mut num_nfts_to_withdraw = self.token_ids_in_pools.len() as u128 * lp / self.lp_supply;
        let mut value_in_fraction_nft = 0u128;
        if num_nfts_to_withdraw * self.lp_supply != lp * self.token_ids_in_pools.len() as u128 {
            num_nfts_to_withdraw += 1;
            let buy_info = self.curve.get_buy_info(
                self.spot_price,
                self.delta,
                1,
                self.fee,
                protocol_fee_multiplier.clone(),
            );
            // num_nfts_to_withdraw - 1 nfts with current spot price
            // the rounded up of fraction nft with spot price after buying 1 nft
            value_in_fraction_nft = (num_nfts_to_withdraw - 1) * self.spot_price + 1 * buy_info.new_spot_price;
            require!(value_in_fraction_nft >= withdrawable_near.clone(), "internal error in handling liquidity");
            value_in_fraction_nft -= withdrawable_near.clone();
        }

        if value_in_fraction_nft > withdrawable_near {
            // TODO: should we allow to withdraw in near only?
            env::panic_str("cannot withdraw as liquidity value in near is too small compared to nft spot price");
        }

        // TODO: take fee 
        withdrawable_near -= value_in_fraction_nft;

        let token_ids = self
            .token_ids_in_pools
            .keys()
            .take(num_nfts_to_withdraw as usize)
            .collect::<Vec<TokenId>>();
        for token_id in &token_ids {
            self.token_ids_in_pools.remove(token_id);
        }

        self.lp_balances.insert(account_id, &(prev_value - lp));
        self.lp_supply -= lp;
        self.near_balance -= withdrawable_near.clone();
        let protocol_fee = U256::from(withdrawable_near) * U256::from(protocol_fee_multiplier) / WAD;
        (protocol_fee.as_u128(), withdrawable_near, token_ids)
    }

    pub fn internal_register_account_lp(& mut self, account_id: &AccountId) {
        if self.lp_balances.get(account_id).is_none() {
            self.lp_balances.insert(account_id, &0u128);
        }
    }

    //only owner functions
    pub(crate) fn assert_owner(&self) {
        if env::predecessor_account_id() != env::predecessor_account_id() {
            env::panic_str("This method can be called only by pool owner")
        }
    }

    fn assert_not_trading_pool(&self) {
        assert!(self.pool_type != PoolType::Trade, "must not be trading pool");
    }

    pub(crate) fn assert_release(&self) {
        let timestamp_sec = env::block_timestamp_ms() / 1000;
        require!(
            self.released_time <= timestamp_sec,
            "Pool liquidity cannot release now"
        );
    }

    pub fn change_spot_price(&mut self, new_spot_price: u128) {
        self.assert_owner();
        self.assert_not_trading_pool();
        self.spot_price = new_spot_price;
    }

    pub fn change_delta(&mut self, new_delta: u128) {
        self.assert_owner();
        self.assert_not_trading_pool();
        self.delta = new_delta;
    }

    pub fn change_fee(&mut self, new_fee: u128) {
        self.assert_owner();
        self.assert_not_trading_pool();
        self.fee = new_fee;
    }

    pub fn change_asset_recipient(&mut self, new_asset_recipient: Option<AccountId>) {
        self.assert_owner();
        self.assert_not_trading_pool();
        self.asset_recipient = new_asset_recipient;
    }
}
