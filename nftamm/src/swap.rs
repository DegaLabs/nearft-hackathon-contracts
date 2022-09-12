use std::{collections::{HashMap, HashSet}, iter::FromIterator};

use crate::*;
use near_sdk::{
    near_bindgen,
    serde::{Deserialize, Serialize},
};

#[repr(u8)]
#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(crate = "near_sdk::serde")]
pub enum SwapType {
    NFTToNear = 0,
    NearToNFT = 1,
}

impl From<u8> for SwapType {
    fn from(val: u8) -> Self {
        match val {
            0u8 => SwapType::NFTToNear,
            1u8 => SwapType::NearToNFT,
            _ => env::panic_str("unknown SwapType")
        }
    }
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, Clone, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Action {
    pool_id: u64,
    swap_type: u8,
    min_output_near: Option<U128>,
    input_token_ids: Vec<TokenId>,
    output_token_ids: Vec<TokenId>,
    num_out_nfts: Option<u64>,
}

#[near_bindgen]
impl Contract {
    // #[payable]
    // pub fn swap_near_for_nfts(
    //     &mut self,
    //     num_nfts: u64,
    //     pool_id: u64,
    //     nft_ids: Option<Vec<TokenId>>,
    // ) {
    //     let account_id = env::predecessor_account_id();
    //     //ensure that the sender account has enough storage staking for storing num_nfts in case if all nft transfer fails
    //     let account_deposit = self.internal_get_account_or_revert(&account_id);
    //     require!(
    //         account_deposit.near_balance
    //             >= (account_deposit.storage_usage as u128
    //                 + num_nfts as u128 * self.storage_per_nft_deposit as u128)
    //                 * env::storage_byte_cost(),
    //         "not enough prepaid near for storage"
    //     );
    //     let (protocol_fee, input_amount, token_ids) =
    //         self.internal_swap_near_for_nfts(pool_id, nft_ids, num_nfts);
    //     let pool = &mut self.pools[pool_id as usize];

    //     self.protocol_fee_credit += protocol_fee;

    //     if pool.asset_recipient.clone().is_some() {
    //         //transfer to asset recipient
    //         Promise::new(pool.asset_recipient.clone().unwrap())
    //             .transfer(input_amount - protocol_fee);
    //     }

    //     //refund to sender
    //     if env::attached_deposit() > input_amount {
    //         Promise::new(account_id.clone()).transfer(env::attached_deposit() - input_amount);
    //     }

    //     //send nfts to recipient
    //     let asset_id = pool.nft_token.clone();
    //     self.transfer_nfts(&account_id, &asset_id, &token_ids);
    // }

    // pub fn swap_nfts_for_near(&mut self, pool_id: u64, nft_ids: Vec<TokenId>, min_near_out: U128) {
    //     let account_id = env::predecessor_account_id();
    //     //ensure that the sender account has enough storage staking for storing num_nfts in case if all nft transfer fails
    //     let (protocol_fee, output_amount) =
    //         self.internal_swap_nfts_for_near(pool_id, &nft_ids, &min_near_out.0);

    //     self.protocol_fee_credit += protocol_fee;
    //     let nft_token: AssetId;
    //     {
    //         let pool = self.pools.get(pool_id as usize).expect("pool id invalid");
    //         nft_token = pool.nft_token.clone();
    //     }

    //     self.internal_withdraw_nft(&account_id, &nft_token, &nft_ids);

    //     if output_amount > 0 {
    //         Promise::new(account_id.clone()).transfer(output_amount);
    //     }
    // }

    fn get_nft_asset_id(&self, pool_id: u64) -> AssetId {
        let pool = self.pools.get(pool_id as usize).expect("pool id invalid");
        pool.nft_token.clone()
    }

    fn internal_swap_with_action(
        &mut self,
        account_id: &AccountId,
        action: &Action,
        cached_token_ids: &mut HashMap<AssetId, HashSet<TokenId>>,
        input_near_value: &Balance,
    ) -> (Balance, Balance) {
        let nft_token = self.get_nft_asset_id(action.pool_id);
        match SwapType::from(action.swap_type) {
            SwapType::NFTToNear => {
                let mut nft_ids = action.input_token_ids.clone();
                if nft_ids.len() == 0 {
                    nft_ids = Vec::from_iter(cached_token_ids.get(&nft_token).unwrap().clone());
                }
                let mut remain_token_ids_to_withdraw = Vec::<TokenId>::new();
                for token_id in &nft_ids {
                    if !cached_token_ids.contains_key(&nft_token.clone()) {
                        remain_token_ids_to_withdraw.push(token_id.clone());
                    } else {
                        if !cached_token_ids.get(&nft_token).unwrap().contains(token_id) {
                            remain_token_ids_to_withdraw.push(token_id.clone());
                        } else {
                            let mut token_set = cached_token_ids.get(&nft_token).unwrap().clone();
                            token_set.remove(token_id);
                            cached_token_ids.insert(nft_token.clone(), token_set);
                        }
                    }
                }
                self.internal_withdraw_nft(account_id, &nft_token, &remain_token_ids_to_withdraw);

                let (protocol_fee, output_amount) = self.internal_swap_nfts_for_near(
                    action.pool_id,
                    &nft_ids,
                    &action.min_output_near.unwrap().0,
                );

                let pool = &mut self.pools[action.pool_id as usize];
                let asset_recipient = pool.asset_recipient.clone();
                if asset_recipient.clone().is_some() {
                    // near pool, deposit nft tok asset recipient
                    for token_id in nft_ids {
                        self.internal_deposit_nft(
                            &asset_recipient.clone().unwrap(),
                            &nft_token,
                            &token_id,
                        );
                    }
                }

                let remain_near_amount = input_near_value + output_amount;

                self.protocol_fee_credit += protocol_fee;
                (protocol_fee, remain_near_amount)
            }
            SwapType::NearToNFT => {
                let nft_ids;
                if action.output_token_ids.len() > 0 {
                    //swap witt output specific token ids
                    require!(
                        action.output_token_ids.len() == action.num_out_nfts.unwrap() as usize,
                        "invalid num out nfts"
                    );
                    nft_ids = Some(action.output_token_ids.clone());
                } else {
                    nft_ids = None;
                }
                let (protocol_fee, input_amount, token_ids) =
                    self.internal_swap_near_for_nfts(action.pool_id, nft_ids, action.num_out_nfts.unwrap());
                self.protocol_fee_credit += protocol_fee;

                let mut token_set = cached_token_ids.get(&nft_token).unwrap_or(&HashSet::new()).clone();
                for token_id in &token_ids {
                    token_set.insert(token_id.clone());
                }
                cached_token_ids.insert(nft_token.clone(), token_set);

                let pool = &self.pools[action.pool_id as usize];

                if pool.asset_recipient.clone().is_some() {
                    //deposit near to asset recipient
                    let mut asset_recipient_deposit =
                        self.internal_get_account_or_revert(&pool.asset_recipient.clone().unwrap());
                    asset_recipient_deposit.near_balance += input_amount - protocol_fee;
                    self.account_deposits.insert(
                        &pool.asset_recipient.clone().unwrap(),
                        &asset_recipient_deposit,
                    );
                }
                let remain_near_amount = input_near_value - input_amount;
                (protocol_fee, remain_near_amount)
            }
        }
    }

    #[payable]
    pub fn swap(&mut self, actions: Vec<Action>) {
        let account_id = env::predecessor_account_id();
        let mut remain_near_amount = env::attached_deposit();
        let mut _protocol_fee = 0u128;
        let mut cached_token_ids = HashMap::<AssetId, HashSet<TokenId>>::new();
        let first_action = actions.get(0).unwrap();
        if first_action.swap_type == SwapType::NFTToNear as u8 {
            require!(first_action.input_token_ids.len() > 0, "inpput token ids invalid");
        }
        for action in &actions {
            (_protocol_fee, remain_near_amount) = self.internal_swap_with_action(
                &account_id,
                action,
                &mut cached_token_ids,
                &remain_near_amount,
            );
        }

        if remain_near_amount > 0 {
            Promise::new(account_id.clone()).transfer(remain_near_amount);
        }
        
        for (nft_token, token_ids) in cached_token_ids.into_iter() {
            if token_ids.len() > 0 {
                self.transfer_nfts(&account_id, &nft_token, &Vec::from_iter(token_ids));
            }
        }

        //should not need to check storage here as swap function only works on assets already deposited
    }
}
