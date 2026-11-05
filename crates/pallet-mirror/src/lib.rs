#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
use scale_info::TypeInfo;
use sp_std::vec::Vec;
use tartarus_primitives::{Balance, DebtId, Deposit, MirrorId, MirrorNft, TokenId};

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
    }

    #[pallet::error]
    pub enum Error<T> {
        MirrorNotFound,
        DebtMirrorNotFound,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(10_000)]
        pub fn deposit(
            origin: OriginFor<T>,
            mirror_id: MirrorId,
            token: TokenId,
            amount: Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            MirrorNfts::<T>::try_mutate(mirror_id, |maybe| {
                let m = maybe.as_mut().ok_or(Error::<T>::MirrorNotFound)?;
                m.deposited.push(Deposit {
                    token,
                    amount,
                    depositor: who.clone(),
                });
                m.price = m.price.saturating_add(amount);
                Ok::<(), DispatchError>(())
            })?;
            Self::deposit_event(Event::MirrorDeposited {
                mirror_id,
                token,
                amount,
                depositor: who,
            });
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn mint_mirror(
            debt_id: DebtId,
            token: TokenId,
            amount: Balance,
            owner: T::AccountId,
        ) -> Result<MirrorId, DispatchError> {
            let id = MirrorCount::<T>::get() + 1;
            let mirror = MirrorNft {
                id,
                debt_id,
                token,
                amount,
                owner: owner.clone(),
                price: 0,
                deposited: Vec::new(),
            };
            MirrorNfts::<T>::insert(id, mirror);
            DebtMirror::<T>::insert(debt_id, id);
            DebtPrincipal::<T>::insert(debt_id, amount);
            MirrorCount::<T>::put(id);
            Self::deposit_event(Event::MirrorMinted {
                mirror_id: id,
                debt_id,
                owner,
            });
            Ok(id)
        }

        pub fn get_mirror(id: MirrorId) -> Option<MirrorNft<T::AccountId>> {
            MirrorNfts::<T>::get(id)
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
    fn mint_and_deposit() {
        let t = frame_system::GenesisConfig::<TestRuntime>::default()
            .build_storage()
            .unwrap();
        let mut ext = sp_io::TestExternalities::new(t);
        ext.execute_with(|| {
            let id = crate::Pallet::<TestRuntime>::mint_mirror(1, 1, 100, 1).unwrap();
            assert_eq!(id, 1);
            assert_eq!(crate::Pallet::<TestRuntime>::principal(1), 100);
            // deposit
            let _ = crate::Pallet::<TestRuntime>::deposit(
                frame_system::RawOrigin::Signed(2).into(),
                id,
                2,
                10,
            );
            assert_eq!(crate::Pallet::<TestRuntime>::aggregate_deposit_value(1), 10);
        });
    }
}
