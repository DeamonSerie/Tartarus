#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
use scale_info::TypeInfo;
use tartarus_primitives::{Balance, CurrencyId};

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    }

    #[pallet::storage]
    pub type TreasuryBalances<T: Config> =
        StorageMap<_, Blake2_128Concat, CurrencyId, Balance, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        TreasuryCredited {
            currency_id: CurrencyId,
            amount: Balance,
        },
        TreasurySpent {
            currency_id: CurrencyId,
            amount: Balance,
            to: T::AccountId,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        InsufficientFunds,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(10_000)]
        pub fn spend(
            origin: OriginFor<T>,
            currency_id: CurrencyId,
            amount: Balance,
            to: T::AccountId,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let bal = TreasuryBalances::<T>::get(currency_id);
            if bal < amount {
                return Err(Error::<T>::InsufficientFunds.into());
            }
            TreasuryBalances::<T>::insert(currency_id, bal - amount);
            Self::deposit_event(Event::TreasurySpent {
                currency_id,
                amount,
                to,
            });
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn credit(currency_id: CurrencyId, amount: Balance) {
            TreasuryBalances::<T>::mutate(currency_id, |b| *b = b.saturating_add(amount));
        }
        pub fn balance(currency_id: CurrencyId) -> Balance {
            TreasuryBalances::<T>::get(currency_id)
        }
    }
}
