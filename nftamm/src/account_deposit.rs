use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{UnorderedMap};
use near_sdk::{env, require, AccountId, Balance, PanicOnDefault, StorageUsage};

use near_contract_standards::non_fungible_token::TokenId;
use crate::*;
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct AccountDeposit {
    pub assets: UnorderedMap<AssetId, UnorderedMap<TokenId, bool>>,
    pub near_balance: Balance,
    pub storage_usage: StorageUsage
}

impl Contract {
    pub(crate) fn internal_get_account_or_revert(&self, account_id: &AccountId) -> AccountDeposit {
        log!("internal_get_account_or_revert {:?}", account_id);
        match self.account_deposits.get(account_id) {
            Some(account_deposit) => account_deposit,
            None => env::panic_str("account unregistered")
        }
    }
    pub(crate) fn internal_deposit_nft(&mut self, account_id: &AccountId, asset_id: &AssetId, token_id: &TokenId) {
        let mut account_deposit = self.internal_get_account_or_revert(account_id);
        match account_deposit.assets.get(asset_id) {
            Some(mut token_ids) => {
                token_ids.insert(token_id, &true);
                account_deposit.assets.insert(asset_id, &token_ids);
            },
            None => {
                let mut token_ids = UnorderedMap::new(StorageKey::AssetDeposit { account_id: account_id.clone(), asset_id: asset_id.clone() });
                token_ids.insert(token_id, &true);
                account_deposit.assets.insert(asset_id, &token_ids);
            }
        }
        self.account_deposits.insert(account_id, &account_deposit);
    }

    pub(crate) fn internal_deposit_nft_with_storage_check(&mut self, account_id: &AccountId, asset_id: &AssetId, token_id: &TokenId) {
        let prev_storage = env::storage_usage();
        self.internal_deposit_nft(account_id, asset_id, token_id);
        self.assert_storage(account_id, prev_storage, None);   
    }

    pub(crate) fn internal_withdraw_nft(&mut self, account_id: &AccountId, asset_id: &AssetId, token_ids: &[TokenId]) {
        let mut account_deposit = self.internal_get_account_or_revert(account_id);
        let mut existing_token_ids = match account_deposit.assets.get(asset_id) {
            Some(token_ids) => token_ids,
            None => env::panic_str("no deposited tokens for withdrawal")
        };

        for token_id in token_ids {
            existing_token_ids.remove(token_id);
        }
        account_deposit.assets.insert(asset_id, &existing_token_ids);
        self.account_deposits.insert(account_id, &account_deposit);
    }

    pub(crate) fn assert_storage(
        &mut self,
        account_id: &AccountId,
        prev_storage: StorageUsage,
        attached_deposit: Option<Balance>,
    ) {
        let attached_deposit = attached_deposit.unwrap_or(0);
        log!("reading account");
        let mut account_deposit = self.internal_get_account_or_revert(account_id);
        log!("done get account");
        account_deposit.storage_usage += self.compute_storage_usage(prev_storage);
        log!("done compute storage");
        account_deposit.near_balance += attached_deposit;
        self.account_deposits.insert(account_id, &account_deposit);
        log!("done insert");
        let storage_cost = (account_deposit.storage_usage as u128) * env::storage_byte_cost();
        require!(account_deposit.near_balance >= storage_cost, "storage usage exceeds near balance");
    }

    pub(crate) fn compute_storage_usage(&self, prev: StorageUsage) -> StorageUsage {
        if env::storage_usage() > prev {
            return env::storage_usage() - prev;
        }
        return 0;
    }
}