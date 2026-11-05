#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

pub type DebtId = u64;
pub type MirrorId = u64;
pub type OubrId = u64;
pub type BundleId = u64;
pub type TokenId = u32;
pub type CurrencyId = u32;
pub type Balance = u128;
pub const MAX_DEPOSITS: usize = 1024;
pub const MAX_BETS: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub enum OubrStatus {
    Open,
    Settled,
    Burned,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub enum BundleStatus {
    Sale,
    Finalized,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct Deposit<AccountId> {
    pub token: TokenId,
    pub amount: Balance,
    pub depositor: AccountId,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct DebtSignal<AccountId> {
    pub oracle: AccountId,
    pub debtor: AccountId,
    pub token: TokenId,
    pub debt_amount: Balance,
    pub dealing_currency: CurrencyId,
    pub timestamp: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub struct MirrorToken {
    pub token_id: TokenId,
    pub supply: Balance,
    pub burned: Balance,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct MirrorNft<AccountId> {
    pub id: MirrorId,
    pub debt_id: DebtId,
    pub tokens: Vec<MirrorToken>,
    pub total_minted: Balance,
    pub burn_target: Balance,
    pub burned_total: Balance,
    pub owner: AccountId,
    pub price: Balance,
    pub deposited: Vec<Deposit<AccountId>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct Bet<AccountId> {
    pub wallet: AccountId,
    pub amount: Balance,
    pub ts: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct Oubr<AccountId> {
    pub id: OubrId,
    pub debt_id: DebtId,
    pub dealing_currency: CurrencyId,
    pub pot_base: Balance,
    pub current_price: Balance,
    pub threshold: Balance,
    pub decay_rate_bp: u32,
    pub bets: Vec<Bet<AccountId>>,
    pub window_end: u64,
    pub status: OubrStatus,
    pub winner: Option<AccountId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct Bundle {
    pub id: BundleId,
    pub debt_id: DebtId,
    pub currency_id: CurrencyId,
    pub amount: Balance,
    pub price: Balance,
    pub sale_end: u64,
    pub unsettled_oubrs: u32,
    pub status: BundleStatus,
}

impl<AccountId> MaxEncodedLen for Deposit<AccountId>
where
    AccountId: MaxEncodedLen,
{
    fn max_encoded_len() -> usize {
        TokenId::max_encoded_len() + Balance::max_encoded_len() + AccountId::max_encoded_len()
    }
}

impl<AccountId> MaxEncodedLen for MirrorNft<AccountId>
where
    AccountId: MaxEncodedLen,
{
    fn max_encoded_len() -> usize {
        MirrorId::max_encoded_len()
            + DebtId::max_encoded_len()
            + 4
            + 10 * MirrorToken::max_encoded_len()
            + Balance::max_encoded_len() * 3
            + AccountId::max_encoded_len()
            + Balance::max_encoded_len()
            + 4
            + MAX_DEPOSITS * Deposit::<AccountId>::max_encoded_len()
    }
}

impl<AccountId> MaxEncodedLen for Bet<AccountId>
where
    AccountId: MaxEncodedLen,
{
    fn max_encoded_len() -> usize {
        AccountId::max_encoded_len() + Balance::max_encoded_len() + u64::max_encoded_len()
    }
}

impl<AccountId> MaxEncodedLen for Oubr<AccountId>
where
    AccountId: MaxEncodedLen,
{
    fn max_encoded_len() -> usize {
        OubrId::max_encoded_len()
            + DebtId::max_encoded_len()
            + CurrencyId::max_encoded_len()
            + Balance::max_encoded_len() * 4
            + u32::max_encoded_len()
            + 4
            + MAX_BETS * Bet::<AccountId>::max_encoded_len()
            + u64::max_encoded_len()
            + OubrStatus::max_encoded_len()
            + AccountId::max_encoded_len() // option
    }
}

impl MaxEncodedLen for Bundle {
    fn max_encoded_len() -> usize {
        BundleId::max_encoded_len()
            + DebtId::max_encoded_len()
            + CurrencyId::max_encoded_len()
            + Balance::max_encoded_len() * 2
            + u64::max_encoded_len()
            + u32::max_encoded_len()
            + BundleStatus::max_encoded_len()
    }
}
