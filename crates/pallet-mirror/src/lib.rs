#![cfg_attr(not(feature = "std"), no_std)]

use codec::Decode;
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
use scale_info::TypeInfo;
use sp_std::vec::Vec;
use tartarus_primitives::{Balance, DebtId, Deposit, MirrorId, MirrorNft, MirrorToken, TokenId};

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::traits::Get;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    }

    #[pallet::storage]
    pub type MirrorCount<T> = StorageValue<_, MirrorId, ValueQuery>;

    #[pallet::storage]
    pub type MirrorNfts<T: Config> =
        StorageMap<_, Blake2_128Concat, MirrorId, MirrorNft<T::AccountId>, OptionQuery>;

    #[pallet::storage]
    pub type DebtMirror<T> = StorageMap<_, Blake2_128Concat, DebtId, MirrorId, OptionQuery>;

    #[pallet::storage]
    pub type DebtPrincipal<T: Config> =
        StorageMap<_, Blake2_128Concat, DebtId, Balance, OptionQuery>;

    #[pallet::storage]
    pub type TokenCount<T> = StorageValue<_, TokenId, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        MirrorMinted {
            mirror_id: MirrorId,
            debt_id: DebtId,
            owner: T::AccountId,
        },
        MirrorDeposited {
            mirror_id: MirrorId,
            token: TokenId,
            amount: Balance,
            depositor: T::AccountId,
        },
        MirrorStarted {
            mirror_id: MirrorId,
            debt_id: DebtId,
            owner: T::AccountId,
        },
        TokenBurned {
            mirror_id: MirrorId,
            token_id: TokenId,
            amount: Balance,
            burner: T::AccountId,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        MirrorNotFound,
        DebtMirrorNotFound,
        DebtNotFound,
        MirrorAlreadyExists,
        InvalidTokenCount,
        InvalidSupply,
        TokenNotFound,
        InsufficientSupply,
        BurnTargetExceeded,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(10_000)]
        pub fn start_mirror(
            origin: OriginFor<T>,
            debt_id: DebtId,
            supplies: Vec<Balance>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(supplies.len() == 10, Error::<T>::InvalidTokenCount);

            let unresolved = DebtPrincipal::<T>::get(debt_id).ok_or(Error::<T>::DebtNotFound)?;
            ensure!(unresolved > 0, Error::<T>::DebtNotFound);

            let minted = unresolved / 2;
            let sum: Balance = supplies.iter().copied().sum();
            ensure!(sum == minted, Error::<T>::InvalidSupply);

            ensure!(
                DebtMirror::<T>::get(debt_id).is_none(),
                Error::<T>::MirrorAlreadyExists
            );

            // allocate 10 global token ids
            let mut counter = TokenCount::<T>::get();
            if counter < 10 {
                counter = 10;
            }
            let mut tokens: Vec<MirrorToken> = Vec::new();
            for s in supplies.iter() {
                counter = counter.saturating_add(1);
                tokens.push(MirrorToken {
                    token_id: counter,
                    supply: *s,
                    burned: 0,
                });
            }
            TokenCount::<T>::put(counter);

            let id = MirrorCount::<T>::get() + 1;
            let mirror = MirrorNft {
                id,
                debt_id,
                tokens,
                total_minted: minted,
                burn_target: unresolved.saturating_sub(minted),
                burned_total: 0,
                owner: who.clone(),
                price: 0,
                deposited: Vec::new(),
            };
            MirrorNfts::<T>::insert(id, mirror);
            DebtMirror::<T>::insert(debt_id, id);
            MirrorCount::<T>::put(id);

            Self::deposit_event(Event::MirrorMinted {
                mirror_id: id,
                debt_id,
                owner: who.clone(),
            });
            Self::deposit_event(Event::MirrorStarted {
                mirror_id: id,
                debt_id,
                owner: who,
            });
            Ok(())
        }

        #[pallet::weight(10_000)]
        pub fn buy(
            origin: OriginFor<T>,
            mirror_id: MirrorId,
            token_index: u32,
            amount: Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            MirrorNfts::<T>::try_mutate(mirror_id, |maybe| {
                let m = maybe.as_mut().ok_or(Error::<T>::MirrorNotFound)?;
                let idx = token_index as usize;
                ensure!(idx < m.tokens.len(), Error::<T>::TokenNotFound);
                let tok = &mut m.tokens[idx];
                ensure!(amount <= tok.supply, Error::<T>::InsufficientSupply);
                tok.supply = tok.supply.saturating_sub(amount);
                let token_id = tok.token_id;
                m.deposited.push(Deposit {
                    token: token_id,
                    amount,
                    depositor: who.clone(),
                });
                m.price = m.price.saturating_add(amount);
                Self::deposit_event(Event::MirrorDeposited {
                    mirror_id,
                    token: token_id,
                    amount,
                    depositor: who.clone(),
                });
                Ok::<(), DispatchError>(())
            })?;
            Ok(())
        }

        #[pallet::weight(10_000)]
        pub fn deposit(
            origin: OriginFor<T>,
            mirror_id: MirrorId,
            token_id: TokenId,
            amount: Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            MirrorNfts::<T>::try_mutate(mirror_id, |maybe| {
                let m = maybe.as_mut().ok_or(Error::<T>::MirrorNotFound)?;
                let idx = m
                    .tokens
                    .iter()
                    .position(|t| t.token_id == token_id)
                    .ok_or(Error::<T>::TokenNotFound)?;
                ensure!(amount <= m.tokens[idx].supply, Error::<T>::InsufficientSupply);
                m.tokens[idx].supply = m.tokens[idx].supply.saturating_sub(amount);
                m.deposited.push(Deposit {
                    token: token_id,
                    amount,
                    depositor: who.clone(),
                });
                m.price = m.price.saturating_add(amount);
                Self::deposit_event(Event::MirrorDeposited {
                    mirror_id,
                    token: token_id,
                    amount,
                    depositor: who.clone(),
                });
                Ok::<(), DispatchError>(())
            })?;
            Ok(())
        }

        #[pallet::weight(10_000)]
        pub fn burn(
            origin: OriginFor<T>,
            mirror_id: MirrorId,
            token_index: u32,
            amount: Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            MirrorNfts::<T>::try_mutate(mirror_id, |maybe| {
                let m = maybe.as_mut().ok_or(Error::<T>::MirrorNotFound)?;
                let idx = token_index as usize;
                ensure!(idx < m.tokens.len(), Error::<T>::TokenNotFound);
                ensure!(amount <= m.tokens[idx].supply, Error::<T>::InsufficientSupply);
                ensure!(
                    m.burned_total.saturating_add(amount) <= m.burn_target,
                    Error::<T>::BurnTargetExceeded
                );
                m.tokens[idx].supply = m.tokens[idx].supply.saturating_sub(amount);
                m.tokens[idx].burned = m.tokens[idx].burned.saturating_add(amount);
                m.burned_total = m.burned_total.saturating_add(amount);
                let token_id = m.tokens[idx].token_id;
                Self::deposit_event(Event::TokenBurned {
                    mirror_id,
                    token_id,
                    amount,
                    burner: who.clone(),
                });
                Ok::<(), DispatchError>(())
            })?;
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn register_debt(debt_id: DebtId, principal: Balance) -> DispatchResult {
            DebtPrincipal::<T>::insert(debt_id, principal);
            Ok(())
        }

        pub fn get_mirror(id: MirrorId) -> Option<MirrorNft<T::AccountId>> {
            MirrorNfts::<T>::get(id)
        }

        pub fn get_token(mirror_id: MirrorId, token: TokenId) -> Option<MirrorToken> {
            let m = Self::get_mirror(mirror_id)?;
            if let Some(t) = m.tokens.iter().find(|t| t.token_id == token) {
                return Some(t.clone());
            }
            let idx = token as usize;
            if idx < m.tokens.len() {
                return Some(m.tokens[idx].clone());
            }
            None
        }

        pub fn get_token_by_index(mirror_id: MirrorId, index: u32) -> Option<MirrorToken> {
            Self::get_mirror(mirror_id)?
                .tokens
                .get(index as usize)
                .cloned()
        }

        pub fn tokens(mirror_id: MirrorId) -> Vec<MirrorToken> {
            Self::get_mirror(mirror_id)
                .map(|m| m.tokens)
                .unwrap_or_default()
        }

        pub fn total_minted(mirror_id: MirrorId) -> Balance {
            Self::get_mirror(mirror_id)
                .map(|m| m.total_minted)
                .unwrap_or(0)
        }

        pub fn burn_target(mirror_id: MirrorId) -> Balance {
            Self::get_mirror(mirror_id)
                .map(|m| m.burn_target)
                .unwrap_or(0)
        }

        pub fn burned_total(mirror_id: MirrorId) -> Balance {
            Self::get_mirror(mirror_id)
                .map(|m| m.burned_total)
                .unwrap_or(0)
        }

        pub fn debt_mirror_id(debt_id: DebtId) -> Option<MirrorId> {
            DebtMirror::<T>::get(debt_id)
        }

        pub fn principal(debt_id: DebtId) -> Balance {
            DebtPrincipal::<T>::get(debt_id).unwrap_or(0)
        }

        pub fn aggregate_deposit_value(debt_id: DebtId) -> Balance {
            DebtMirror::<T>::get(debt_id)
                .and_then(|mid| MirrorNfts::<T>::get(mid))
                .map(|m| m.price)
                .unwrap_or(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::pallet::*;
    use super::*;
    use frame_support::{construct_runtime, parameter_types, traits::ConstU32};
    use sp_core::H256;
    use sp_runtime::{
        traits::{BlakeTwo256, IdentityLookup},
        BuildStorage,
    };

    type Block = frame_system::mocking::MockBlock<TestRuntime>;
    construct_runtime!(
        pub enum TestRuntime {
            System: frame_system,
            Mirror: crate,
        }
    );

    parameter_types! {
        pub const BlockHashCount: u64 = 250;
    }

    impl frame_system::Config for TestRuntime {
        type BaseCallFilter = frame_support::traits::Everything;
        type BlockWeights = ();
        type BlockLength = ();
        type DbWeight = ();
        type RuntimeOrigin = RuntimeOrigin;
        type RuntimeCall = RuntimeCall;
        type Nonce = u64;
        type Hash = H256;
        type Hashing = BlakeTwo256;
        type AccountId = u64;
        type Lookup = IdentityLookup<Self::AccountId>;
        type Block = Block;
        type RuntimeEvent = RuntimeEvent;
        type BlockHashCount = BlockHashCount;
        type Version = ();
        type PalletInfo = PalletInfo;
        type AccountData = ();
        type SystemWeightInfo = ();
        type SS58Prefix = ();
        type OnSetCode = ();
        type MaxConsumers = ConstU32<16>;
        type RuntimeTask = ();
        type OnNewAccount = ();
        type OnKilledAccount = ();
        type ExtensionsWeightInfo = ();
        type SingleBlockMigrations = ();
        type MultiBlockMigrator = ();
        type PreInherents = ();
        type PostInherents = ();
        type PostTransactions = ();
    }

    impl crate::Config for TestRuntime {
        type RuntimeEvent = RuntimeEvent;
    }

    #[test]
    fn start_and_buy_burn() {
        let t = frame_system::GenesisConfig::<TestRuntime>::default()
            .build_storage()
            .unwrap();
        let mut ext = sp_io::TestExternalities::new(t);
        ext.execute_with(|| {
            // register debt principal 800
            crate::Pallet::<TestRuntime>::register_debt(1, 800).unwrap();
            let supplies = vec![40u128; 10]; // sum 400 = 800/2
            crate::Pallet::<TestRuntime>::start_mirror(
                frame_system::RawOrigin::Signed(1).into(),
                1,
                supplies,
            )
            .unwrap();
            let mid = crate::Pallet::<TestRuntime>::debt_mirror_id(1).unwrap();
            assert_eq!(mid, 1);
            assert_eq!(crate::Pallet::<TestRuntime>::principal(1), 800);
            assert_eq!(crate::Pallet::<TestRuntime>::total_minted(mid), 400);
            assert_eq!(crate::Pallet::<TestRuntime>::burn_target(mid), 400);
            assert_eq!(crate::Pallet::<TestRuntime>::tokens(mid).len(), 10);
            // buy
            crate::Pallet::<TestRuntime>::buy(
                frame_system::RawOrigin::Signed(2).into(),
                mid,
                0,
                10,
            )
            .unwrap();
            assert_eq!(crate::Pallet::<TestRuntime>::get_token(mid, 0).unwrap().supply, 30);
            assert_eq!(crate::Pallet::<TestRuntime>::aggregate_deposit_value(1), 10);
            // burn
            crate::Pallet::<TestRuntime>::burn(
                frame_system::RawOrigin::Signed(3).into(),
                mid,
                0,
                5,
            )
            .unwrap();
            assert_eq!(crate::Pallet::<TestRuntime>::burned_total(mid), 5);
            assert_eq!(crate::Pallet::<TestRuntime>::get_token(mid, 0).unwrap().supply, 25);
        });
    }
}
