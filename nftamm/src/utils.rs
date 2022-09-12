use near_sdk::{near_bindgen, PromiseResult};

use crate::{AssetId, *};

use crate::nft_core::{ext_nft_core, ext_self};

#[near_bindgen]
impl Contract {
    #[private]
    pub fn nft_transfer_resolve(
        &mut self,
        account_id: near_sdk::AccountId,
        asset_id: AssetId,
        token_id: TokenId,
    ) {
        // assert_eq!(
        //     env::promise_results_count(),
        //     1,
        //     "{}",
        //     "nft transfer failed"
        // );
        match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(_) => {}
            PromiseResult::Failed => {
                //insert nft into user accoutn deposit
                self.internal_deposit_nft(&account_id, &asset_id, &token_id);
            }
        };
    }
}

impl Contract {
    pub(crate) fn transfer_nfts(
        &mut self,
        receiver_id: &AccountId,
        asset_id: &AssetId,
        token_ids: &Vec<TokenId>,
    ) {
        for token_id in token_ids {
            let this_contract = env::current_account_id();
            ext_nft_core::ext(asset_id.clone())
                .with_static_gas(GAS_FOR_NFT_TRANSFER_CALL)
                .with_attached_deposit(1)
                .nft_transfer(receiver_id.clone(), token_id.clone(), None, None)
                .then(ext_self::ext(this_contract)
                    .with_static_gas(GAS_FOR_NFT_TRANSFER_CALL)
                    .nft_transfer_resolve(receiver_id.clone(), asset_id.clone(), token_id.clone()));
        }
    }
}
