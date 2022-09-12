use near_contract_standards::storage_management::{
    StorageBalance, StorageBalanceBounds, StorageManagement,
};
use near_sdk::json_types::U128;
use near_sdk::{assert_one_yocto, env, AccountId, Balance, Promise};

use crate::*;

impl Contract {
    /// Internal method that returns the Account ID and the balance in case the account was
    /// unregistered.
    pub fn internal_storage_unregister(
        &mut self,
        _force: Option<bool>,
    ) -> Option<(AccountId, Balance)> {
        assert_one_yocto();
        None
    }

    fn internal_storage_balance_of(&self, account_id: &AccountId) -> Option<StorageBalance> {
        if self.account_deposits.get(account_id).is_some() {
            let storage_account = self.account_deposits.get(account_id).unwrap();
            Some(StorageBalance {
                total: U128(storage_account.near_balance),
                available: U128(self.storage_available(account_id.clone()).0),
            })
        } else {
            None
        }
    }

    pub fn internal_register_account(
        &mut self,
        account_id: &AccountId,
        amount: &Balance,
    ) -> Balance {
        let init_storage = env::storage_usage();
        if self.account_deposits.get(account_id).is_none() {
            self.account_deposits.insert(
                account_id,
                &AccountDeposit {
                    assets: UnorderedMap::new(StorageKey::AccountDepositAsset {
                        account_id: account_id.clone(),
                    }),
                    near_balance: 0,
                    storage_usage: 0,
                },
            );
        }
        let mut storage_account = self.account_deposits.get(account_id).unwrap();
        storage_account.near_balance += amount;

        self.account_deposits.insert(account_id, &storage_account);

        let storage_used = env::storage_usage() - init_storage;
        let mut storage_account = self.account_deposits.get(account_id).unwrap();
        storage_account.storage_usage += storage_used;
        self.account_deposits.insert(account_id, &storage_account);
        self.assert_storage(account_id, env::storage_usage(), None);

        0
    }
}

#[near_bindgen]
impl StorageManagement for Contract {
    // `registration_only` doesn't affect the implementation for vanilla fungible token.
    #[allow(unused_variables)]
    #[payable]
    fn storage_deposit(
        &mut self,
        account_id: Option<AccountId>,
        registration_only: Option<bool>,
    ) -> StorageBalance {
        let amount: Balance = env::attached_deposit();
        let account_id = account_id
            .map(|a| a.into())
            .unwrap_or_else(|| env::predecessor_account_id());

        let bounds_for_account = self.storage_balance_bounds_for_account(account_id.clone());
        let min = bounds_for_account.min.0;

        if amount < min {
            env::panic_str("The attached deposit is less than the minimum storage balance");
        }

        let refund = self.internal_register_account(&account_id, &amount);

        if refund > 0 {
            Promise::new(env::predecessor_account_id()).transfer(refund);
        }

        self.internal_storage_balance_of(&account_id).unwrap()
    }

    /// While storage_withdraw normally allows the caller to retrieve `available` balance, the basic
    /// Fungible Token implementation sets storage_balance_bounds.min == storage_balance_bounds.max,
    /// which means available balance will always be 0. So this implementation:
    /// * panics if `amount > 0`
    /// * never transfers Ⓝ to caller
    /// * returns a `storage_balance` struct if `amount` is 0
    fn storage_withdraw(&mut self, amount: Option<U128>) -> StorageBalance {
        assert_one_yocto();
        let predecessor_account_id = env::predecessor_account_id();
        if let Some(storage_balance) = self.internal_storage_balance_of(&predecessor_account_id) {
            match amount {
                Some(amount) if amount.0 > 0 => {
                    env::panic_str("The amount is greater than the available storage balance");
                }
                _ => storage_balance,
            }
        } else {
            env::panic_str(
                format!("The account {} is not registered", &predecessor_account_id).as_str(),
            );
        }
    }

    fn storage_unregister(&mut self, force: Option<bool>) -> bool {
        self.internal_storage_unregister(force).is_some()
    }

    fn storage_balance_bounds(&self) -> StorageBalanceBounds {
        let min = Balance::from(self.storage_per_account_creation + self.storage_per_nft_deposit)
            * env::storage_byte_cost();
        let max = min;
        StorageBalanceBounds {
            min: U128(min),
            max: Some(U128(max)),
        }
    }

    fn storage_balance_of(&self, account_id: AccountId) -> Option<StorageBalance> {
        self.internal_storage_balance_of(&account_id)
    }
}

#[near_bindgen]
impl Contract {
    pub fn storage_balance_bounds_for_account(
        &self,
        _account_id: AccountId,
    ) -> StorageBalanceBounds {
        let required_storage_balance =
            Balance::from(self.storage_per_account_creation + self.storage_per_nft_deposit)
                * env::storage_byte_cost();
        let min = Balance::from(self.storage_per_account_creation + self.storage_per_nft_deposit)
            * env::storage_byte_cost();
        StorageBalanceBounds {
            min: U128(min),
            max: Some(U128(required_storage_balance)),
        }
    }

    pub fn storage_available(&self, account_id: AccountId) -> U128 {
        let storage_account = self.account_deposits.get(&account_id);
        match storage_account {
            Some(storage_account) => {
                let usage = storage_account.storage_usage as u128 * env::storage_byte_cost();
                if storage_account.near_balance >= usage {
                    U128(storage_account.near_balance - usage)
                } else {
                    U128(0)
                }
            }
            None => U128(0),
        }
    }
}
