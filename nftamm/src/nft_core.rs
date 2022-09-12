use crate::*;
use near_sdk::{ext_contract};

#[ext_contract(ext_nft_core)]
pub trait NFTCore {
    fn nft_transfer(
        &mut self,
        receiver_id: AccountId,
        token_id: TokenId,
        approval_id: Option<u64>,
        memo: Option<String>,
    );
}

#[ext_contract(ext_self)]
pub trait NonFungibleTokenResolver {
    fn nft_transfer_resolve(&mut self, account_id: AccountId, asset_id: AssetId, token_id: TokenId);
}
