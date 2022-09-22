/*!
Fungible Token implementation with JSON serialization.
NOTES:
  - The maximum balance value is limited by U128 (2**128 - 1).
  - JSON calls should pass U128 as a base-10 string. E.g. "100".
  - The contract optimizes the inner trie structure by hashing account IDs. It will prevent some
    abuse of deep tries. Shouldn't be an issue, once NEAR clients implement full hashing of keys.
  - The contract tracks the change in storage before and after the call. If the storage increases,
    the contract requires the caller of the contract to attach enough deposit to the function call
    to cover the storage cost.
    This is done to prevent a denial of service attack on the contract by taking all available storage.
    If the storage decreases, the contract will issue a refund for the cost of the released storage.
    The unused tokens from the attached deposit are also refunded, so it's safe to
    attach more deposit than required.
  - To prevent the deployed contract from being modified or deleted, it should not have any access
    keys on its account.
*/
use account_deposit::AccountDeposit;
use curves::curve::BondingCurve;
use near_contract_standards::non_fungible_token::TokenId;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::UnorderedMap;
use near_sdk::json_types::U128;
use near_sdk::{
    env, log, near_bindgen, require, AccountId, Balance, BorshStorageKey, Gas, PanicOnDefault,
    Promise, StorageUsage, assert_one_yocto,
};
use pair::{Pair, PoolType};

use crate::curves::WAD;
use crate::pair::MAX_FEE;
const GAS_FOR_RESOLVE_TRANSFER: Gas = Gas(5_000_000_000_000);
const GAS_FOR_NFT_TRANSFER_CALL: Gas = Gas(25_000_000_000_000 + GAS_FOR_RESOLVE_TRANSFER.0);

mod account_deposit;
pub mod curves;
mod nft_core;
pub mod pair;
mod receiver;
mod storage_impl;
mod swap;
mod utils;
pub mod view;
mod multi_lp;

pub type AssetId = AccountId;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    pub governance_id: AccountId,
    pub protocol_fee_receiver_id: AccountId,
    pub protocol_fee_credit: Balance,
    pub pools: Vec<Pair>,
    pub protocol_fee_multiplier: Balance,
    pub account_deposits: UnorderedMap<AccountId, AccountDeposit>,
    pub storage_per_account_creation: StorageUsage,
    pub storage_per_nft_deposit: StorageUsage,
    pub storage_per_pair_creation: StorageUsage,
    pub created_pool_ids: UnorderedMap<AccountId, Vec<u64>>,
}

#[derive(BorshStorageKey, BorshSerialize)]
enum StorageKey {
    AccountDeposits,
    AccountDepositAsset {
        account_id: AccountId,
    },
    AssetDeposit {
        account_id: AccountId,
        asset_id: AssetId,
    },
    CreatedPoolIds,
    TokenIdsInPools {
        pool_id: u64,
    },
    PoolShare {
        pool_id: u64
    }
}

#[near_bindgen]
impl Contract {
    /// Initializes the contract with the given total supply owned by the given `owner_id` with
    /// the given fungible token metadata.
    #[init]
    pub fn new(
        governance_id: Option<AccountId>,
        protocol_fee_receiver_id: Option<AccountId>,
        protocol_fee_multiplier: Option<U128>,
    ) -> Self {
        require!(!env::state_exists(), "Already initialized");
        let caller = env::predecessor_account_id();
        let mut this = Self {
            pools: vec![],
            protocol_fee_multiplier: protocol_fee_multiplier.unwrap_or(U128(10u128.pow(17))).0,
            governance_id: governance_id.unwrap_or(caller.clone()),
            protocol_fee_receiver_id: protocol_fee_receiver_id.unwrap_or(caller.clone()),
            account_deposits: UnorderedMap::new(StorageKey::AccountDeposits),
            storage_per_account_creation: 0,
            storage_per_nft_deposit: 0,
            storage_per_pair_creation: 0,
            created_pool_ids: UnorderedMap::new(StorageKey::CreatedPoolIds),
            protocol_fee_credit: 0,
        };
        this.measure_storage_usage();
        this
    }

    fn measure_storage_usage(&mut self) {
        let mut prev_storage = env::storage_usage();
        let account_id = AccountId::new_unchecked("a".repeat(64));
        let asset_id = AccountId::new_unchecked("a".repeat(64));
        let account_deposit = AccountDeposit {
            near_balance: 0u128,
            storage_usage: 0,
            assets: UnorderedMap::new(StorageKey::AccountDepositAsset {
                account_id: account_id.clone(),
            }),
        };

        self.account_deposits.insert(&account_id, &account_deposit);
        self.storage_per_account_creation = env::storage_usage() - prev_storage;
        prev_storage = env::storage_usage();

        let mut account_deposit = self.account_deposits.get(&account_id).unwrap();
        account_deposit.assets.insert(
            &asset_id,
            &UnorderedMap::new(StorageKey::AssetDeposit {
                account_id: account_id.clone(),
                asset_id: asset_id.clone(),
            }),
        );
        let mut deposit_map = account_deposit.assets.get(&asset_id).unwrap();
        let token_id = "a".repeat(64);
        deposit_map.insert(&token_id, &true);
        account_deposit.assets.insert(&asset_id, &deposit_map);
        self.account_deposits.insert(&account_id, &account_deposit);
        self.storage_per_nft_deposit = env::storage_usage() - prev_storage;

        //remove account
        self.account_deposits.remove(&account_id);

        prev_storage = env::storage_usage();
        let pool_id = 0;
        let new_pair = Pair::new(
            BondingCurve::LinearCurve,
            PoolType::Trade,
            asset_id.clone(),
            10u128,
            10u128,
            10u128,
            account_id.clone(),
            None,
            0u128,
            0,
            0,
        );
        self.pools.push(new_pair);
        let mut created_list = Vec::<u64>::new();
        created_list.push(pool_id);
        self.created_pool_ids.insert(&account_id, &created_list);
        self.storage_per_pair_creation = env::storage_usage() - prev_storage;

        self.created_pool_ids.remove(&account_id);
        self.pools = vec![];
    }

    pub fn set_protocol_fee_receiver(&mut self, account_id: AccountId) {
        require!(
            env::predecessor_account_id() == self.governance_id.clone(),
            "only governance"
        );
        self.protocol_fee_receiver_id = account_id;
    }

    #[payable]
    pub fn create_pair(
        &mut self,
        pool_type: u8,
        bonding_curve: u8,
        asset_id: AssetId,
        spot_price: U128,
        delta: U128,
        fee: U128,
        asset_recipient: Option<AccountId>,
        initial_token_ids: Vec<TokenId>,
        locked_til: u64,
    ) -> u64 {
        log!(
            "trade fee {:?}, max fee {:?}, wad {:?}",
            fee,
            U128(MAX_FEE),
            U128(WAD)
        );
        let prev_storage = env::storage_usage();
        let account_id = env::predecessor_account_id();
        let pool_id = self.pools.len();
        let new_pair = Pair::new(
            bonding_curve.into(),
            pool_type.into(),
            asset_id.clone(),
            spot_price.0,
            delta.0,
            fee.0,
            account_id.clone(),
            asset_recipient.clone(),
            0u128,
            locked_til,
            pool_id as u64,
        );
        log!("Pool created");
        self.pools.push(new_pair);
        match self.created_pool_ids.get(&account_id) {
            Some(mut pool_ids) => {
                pool_ids.push(pool_id as u64);
                self.created_pool_ids.insert(&account_id, &pool_ids);
            }
            None => {
                log!("creating new vector");
                let mut pool_ids = Vec::<u64>::new();
                pool_ids.push(pool_id as u64);
                self.created_pool_ids.insert(&account_id, &pool_ids);
            }
        }
        log!("done added pool");

        if asset_recipient.clone().is_some() {
            let acc = asset_recipient.unwrap();
            if self
                .account_deposits
                .get(&acc)
                .is_none()
            {
                self.account_deposits.insert(
                    &acc,
                    &AccountDeposit {
                        assets: UnorderedMap::new(StorageKey::AccountDepositAsset {
                            account_id: acc.clone(),
                        }),
                        near_balance: 0,
                        storage_usage: 0,
                    },
                );
            }
        }

        self.internal_withdraw_nft(&account_id, &asset_id, &initial_token_ids);
        let pool = &mut self.pools[pool_id];
        pool.internal_register_account_lp(&account_id);
        log!("depositing near");
        pool.deposit_and_mint_lp(account_id.clone(), account_id.clone(), &initial_token_ids, &env::attached_deposit());
        self.assert_storage(&account_id, prev_storage, Some(0));
        log!("done assert storage");
        pool_id as u64
    }

    #[payable]
    pub fn add_liquidity(&mut self, pool_id: u64, token_ids: Vec<TokenId>) {
        let prev_storage = env::storage_usage();
        let account_id = env::predecessor_account_id();
        let pool = &mut self.pools[pool_id as usize];
        pool.deposit_and_mint_lp(account_id.clone(), account_id.clone(), &token_ids, &env::attached_deposit());
        self.assert_storage(&account_id, prev_storage, Some(0));
    }

    #[payable]
    pub fn remove_liquidity(&mut self, pool_id: u64, lp: U128) {
        assert_one_yocto();
        let account_id = env::predecessor_account_id();
        let nft_token = self.get_nft_asset_id(pool_id);
        let pool = &mut self.pools[pool_id as usize];
        let (protocol_fee, withdrawnable_near, token_ids) = pool.burn_lp(&account_id, lp.0, self.protocol_fee_multiplier);
        self.protocol_fee_credit += protocol_fee;
        Promise::new(account_id.clone()).transfer(withdrawnable_near);
        self.transfer_nfts(&account_id, &nft_token, &token_ids);
    }

    #[payable]
    pub fn withdraw_near(&mut self, pool_id: u64, near_amount: U128) {
        let prev_storage = env::storage_usage();
        let account_id = env::predecessor_account_id();
        let pool = &mut self.pools[pool_id as usize];
        pool.withdraw_near(&near_amount.0);
        self.assert_storage(&account_id, prev_storage, Some(env::attached_deposit()));

        Promise::new(account_id.clone()).transfer(near_amount.0);
    }

    #[payable]
    pub fn withdraw_nfts(&mut self, pool_id: u64, token_ids: Vec<TokenId>) {
        let prev_storage = env::storage_usage();
        let account_id = env::predecessor_account_id();
        let pool = &mut self.pools[pool_id as usize];
        pool.withdraw_nfts(&token_ids);

        let asset_id = pool.nft_token.clone();
        self.transfer_nfts(&account_id, &asset_id, &token_ids);
        {
            self.assert_storage(&account_id, prev_storage, Some(env::attached_deposit()));
        }
    }
    #[payable]
    pub fn withdraw_nfts_from_deposit(&mut self, asset_id: AssetId, token_ids: Vec<TokenId>) {
        require!(
            env::attached_deposit() >= token_ids.len() as u128,
            "require attachment"
        );
        let account_id = env::predecessor_account_id();
        self.internal_withdraw_nft(&account_id, &asset_id, &token_ids);

        self.transfer_nfts(&account_id, &asset_id, &token_ids);
    }
}

impl Contract {
    fn internal_swap_near_for_nfts(
        &mut self,
        pool_id: u64,
        nft_ids: Option<Vec<TokenId>>,
        num_nfts: u64,
    ) -> (Balance, Balance, Vec<TokenId>) {
        let pool = &mut self.pools[pool_id as usize];
        let protocol_fee: u128;
        let input_amount: u128;
        let token_ids: Vec<TokenId>;
        if nft_ids.is_none() {
            (protocol_fee, input_amount, token_ids) = pool.swap_near_for_any_nfts(
                env::attached_deposit(),
                num_nfts,
                self.protocol_fee_multiplier,
            );
        } else {
            require!(
                num_nfts as usize == nft_ids.clone().unwrap().len(),
                "invalid nft size"
            );
            (protocol_fee, input_amount) = pool.swap_near_for_specific_nfts(
                env::attached_deposit(),
                &nft_ids.clone().unwrap(),
                self.protocol_fee_multiplier,
            );
            token_ids = nft_ids.unwrap();
        }

        (protocol_fee, input_amount, token_ids)
    }

    fn internal_swap_nfts_for_near(
        &mut self,
        pool_id: u64,
        nft_ids: &Vec<TokenId>,
        min_near_out: &Balance,
    ) -> (Balance, Balance) {
        let pool = &mut self.pools[pool_id as usize];
        let (protocol_fee, output_amount) =
            pool.swap_nfts_for_near(&nft_ids, min_near_out.clone(), self.protocol_fee_multiplier);
        (protocol_fee, output_amount)
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::{testing_env, Balance};

    use super::*;

    fn governance_id() -> AccountId {
        AccountId::new_unchecked("governance.near".to_string())
    }

    fn protocol_fee_receiver_id() -> AccountId {
        AccountId::new_unchecked("protocol_fee_receiver_id.near".to_string())
    }

    fn contract_id() -> AccountId {
        AccountId::new_unchecked("contract_id.near".to_string())
    }

    fn user1() -> AccountId {
        AccountId::new_unchecked("user1.near".to_string())
    }

    fn protocol_fee_multiplier() -> Balance {
        10u128.pow(16) //1 %
    }

    // const TOTAL_SUPPLY: Balance = 1_000_000_000_000_000;

    fn get_context(predecessor_account_id: AccountId) -> VMContextBuilder {
        let mut builder = VMContextBuilder::new();
        builder
            .current_account_id(contract_id())
            .signer_account_id(predecessor_account_id.clone())
            .predecessor_account_id(predecessor_account_id);
        builder
    }

    #[test]
    fn test_new() {
        let mut context = get_context(user1());
        testing_env!(context.build());
        let contract = Contract::new(
            governance_id().into(),
            protocol_fee_receiver_id().into(),
            Some(protocol_fee_multiplier().into()),
        );
        testing_env!(context.is_view(true).build());
        assert_eq!(contract.pools.len(), 0);
        assert_eq!(contract.governance_id, governance_id());
        assert_eq!(
            contract.protocol_fee_receiver_id,
            protocol_fee_receiver_id()
        );
        assert_eq!(contract.protocol_fee_multiplier, protocol_fee_multiplier());
        assert_ne!(contract.storage_per_account_creation, 0);
    }

    // #[test]
    // #[should_panic(expected = "The contract is not initialized")]
    // fn test_default() {
    //     let context = get_context(accounts(1));
    //     testing_env!(context.build());
    //     let _contract = Contract::default();
    // }

    // #[test]
    // fn test_transfer() {
    //     let mut context = get_context(accounts(2));
    //     testing_env!(context.build());
    //     let mut contract = Contract::new_default_meta(accounts(2).into(), TOTAL_SUPPLY.into());
    //     testing_env!(context
    //         .storage_usage(env::storage_usage())
    //         .attached_deposit(contract.storage_balance_bounds().min.into())
    //         .predecessor_account_id(accounts(1))
    //         .build());
    //     // Paying for account registration, aka storage deposit
    //     contract.storage_deposit(None, None);

    //     testing_env!(context
    //         .storage_usage(env::storage_usage())
    //         .attached_deposit(1)
    //         .predecessor_account_id(accounts(2))
    //         .build());
    //     let transfer_amount = TOTAL_SUPPLY / 3;
    //     contract.ft_transfer(accounts(1), transfer_amount.into(), None);

    //     testing_env!(context
    //         .storage_usage(env::storage_usage())
    //         .account_balance(env::account_balance())
    //         .is_view(true)
    //         .attached_deposit(0)
    //         .build());
    //     assert_eq!(
    //         contract.ft_balance_of(accounts(2)).0,
    //         (TOTAL_SUPPLY - transfer_amount)
    //     );
    //     assert_eq!(contract.ft_balance_of(accounts(1)).0, transfer_amount);
    // }
}
