use std::collections::HashMap;

use near_contract_standards::non_fungible_token::TokenId;
use near_sdk::{serde::{Serialize, Deserialize}};

use crate::{*, pair::{PoolType}, curves::{errorcodes::CurveErrorCode, curve::BondingCurve, BuyInfo}};
#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, Clone, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct PairInfo {
    pub curve_type: BondingCurve,
    pub pool_type: PoolType,
    pub nft_token: AssetId,
    pub spot_price: U128,
    pub delta: U128,
    pub fee: U128,
    pub owner: AccountId,
    // If set to none, NFTs/tokens sent by traders during trades will be sent to the pair.
    // Otherwise, assets will be sent to the set address. Not available for TRADE pools
    pub asset_recipient: Option<AccountId>,
    pub near_balance: U128,
    pub pool_token_ids: Vec<TokenId>,
    pub pool_id: u64
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, Clone, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct BuyInfoPublic {
    pub error_code: CurveErrorCode,
    pub new_spot_price: U128,
    pub new_delta: U128,
    pub input_value: U128,
    pub protocol_fee: U128,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, Clone, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct MetaData {
    pub governance_id: AccountId,
    pub protocol_fee_receiver_id: AccountId,
    pub protocol_fee_credit: U128,
    pub pools_acount: u64,
    pub protocol_fee_multiplier: U128,
    pub storage_per_account_creation: StorageUsage,
    pub storage_per_nft_deposit: StorageUsage,
    pub storage_per_pair_creation: StorageUsage,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, Clone, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct SellInfoPublic {
    pub error_code: CurveErrorCode,
    pub new_spot_price: U128,
    pub new_delta: U128,
    pub output_value: U128,
    pub protocol_fee: U128,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, Clone, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct AccountInfo {
    pub deposits: HashMap<AssetId, Vec<TokenId>>,
    pub near_balance: U128,
    pub storage_usage: StorageUsage
}

#[near_bindgen]
impl Contract {
    pub fn get_protocol_fee_multiplier(&self) -> u128 {
        self.protocol_fee_multiplier
    }

    pub fn get_buy_nft_quote(&self, pool_id: u64, num_nfts: u64) -> (CurveErrorCode, U128, U128, U128, U128) {
        let pair = self.pools.get(pool_id as usize).unwrap();
        let buy_info = pair.curve.get_buy_info(pair.spot_price, pair.delta, num_nfts, pair.fee, self.get_protocol_fee_multiplier());
        (buy_info.error_code, buy_info.new_spot_price.into(), buy_info.new_delta.into(), buy_info.input_value.as_u128().into(), buy_info.protocol_fee.as_u128().into())
    }

    pub fn get_sell_nft_quote(&self, pool_id: u64, num_nfts: u64) -> (CurveErrorCode, U128, U128, U128, U128) {
        let pair = self.pools.get(pool_id as usize).unwrap();
        let sell_info = pair.curve.get_sell_info(pair.spot_price, pair.delta, num_nfts, pair.fee, self.get_protocol_fee_multiplier());
        (sell_info.error_code, sell_info.new_spot_price.into(), sell_info.new_delta.into(), sell_info.output_value.as_u128().into(), sell_info.protocol_fee.as_u128().into())
    }

    pub fn get_all_held_ids(&self, pool_id: u64) -> Vec<TokenId> {
        let pair = self.pools.get(pool_id as usize).unwrap();
        pair.token_ids_in_pools.keys_as_vector().to_vec()
    }

    fn pool_to_pair_info(&self, pair: &Pair) -> PairInfo {
        PairInfo { pool_id: pair.pool_id, curve_type: pair.curve.curve_type, pool_type: pair.pool_type, nft_token: pair.nft_token.clone(), spot_price: pair.spot_price.into(), delta: pair.delta.into(), fee: pair.fee.into(), owner: pair.owner.clone(), asset_recipient: pair.asset_recipient.clone(), near_balance: pair.near_balance.into(), pool_token_ids: self.get_all_held_ids(pair.pool_id) }
    }

    pub fn get_pool_info(&self, pool_id: u64) -> PairInfo {
        let pair = self.pools.get(pool_id as usize).unwrap();
        self.pool_to_pair_info(pair)
    }

    pub fn get_pools_infos(&self, pool_ids: Vec<u64>) -> Vec<PairInfo> {
        let mut pairs = Vec::<PairInfo>::new();
        for pool_id in &pool_ids {
            pairs.push(self.get_pool_info(pool_id.clone()));
        }
        pairs
    }

    pub fn get_pool_count(&self) -> u64 {
        self.pools.len() as u64
    }

    pub fn get_pools(&self, from_index: Option<u64>, limit: Option<u64>) -> Vec<PairInfo> {
        let from = from_index.unwrap_or(0);
        if from >= self.pools.len() as u64 {
            return vec![];
        } 

        let limit = limit.unwrap_or(u64::MAX);
        require!(limit != 0, "Cannot provide limit of 0.");

        self.pools
            .iter()
            .skip(from as usize)
            .take(limit as usize)
            .map(|p| self.pool_to_pair_info(p))
            .collect::<Vec<_>>()
    }

    pub fn get_deposits(&self, account_id: AccountId) -> AccountInfo {
        let mut hash_map = HashMap::<AssetId, Vec<TokenId>>::new();
        let account_deposit = self.internal_get_account_or_revert(&account_id);
        for asset_id in account_deposit.assets.keys_as_vector().iter() {
            let held_ids = account_deposit.assets.get(&asset_id).unwrap().keys_as_vector().to_vec();
            hash_map.insert(asset_id, held_ids);
        }

        AccountInfo { deposits: hash_map, near_balance: account_deposit.near_balance.into(), storage_usage: account_deposit.storage_usage }                                        
    }

    pub fn get_buy_info(
        &self,
        pool_id: u64,
        num_items: u64,
    ) -> BuyInfoPublic {
        let pool = &self.pools[pool_id as usize];
        let current_spot_price = pool.spot_price;
        let current_delta = pool.delta;
        let buy_info = pool.curve.get_buy_info(
            current_spot_price,
            current_delta,
            num_items,
            pool.fee,
            self.protocol_fee_multiplier,
        );
        BuyInfoPublic { error_code: buy_info.error_code, new_spot_price: buy_info.new_spot_price.into(), new_delta: buy_info.new_delta.into(), input_value: buy_info.input_value.as_u128().into(), protocol_fee: buy_info.protocol_fee.as_u128().into() }
    }

    pub fn get_sell_info(
        &self, 
        pool_id: u64,
        num_items: u64
    ) -> SellInfoPublic {
        let pool = &self.pools[pool_id as usize];
        let sell_info = pool.curve.get_sell_info(pool.spot_price, pool.delta, num_items, pool.fee, self.protocol_fee_multiplier);
        SellInfoPublic { error_code: sell_info.error_code, new_spot_price: sell_info.new_spot_price.into(), new_delta: sell_info.new_delta.into(), output_value: sell_info.output_value.as_u128().into(), protocol_fee: sell_info.protocol_fee.as_u128().into() }
    }

    pub fn get_metadata(&self) -> MetaData {
        MetaData { governance_id: self.governance_id.clone(), protocol_fee_receiver_id: self.protocol_fee_receiver_id.clone(), protocol_fee_credit: self.protocol_fee_credit.into(), pools_acount: self.pools.len() as u64, protocol_fee_multiplier: self.protocol_fee_multiplier.into(), storage_per_account_creation: self.storage_per_account_creation, storage_per_nft_deposit: self.storage_per_nft_deposit, storage_per_pair_creation: self.storage_per_pair_creation }
    }

    pub fn get_nft_asset_id(&self, pool_id: u64) -> AssetId {
        let pool = self.pools.get(pool_id as usize).expect("pool id invalid");
        pool.nft_token.clone()
    }
}
