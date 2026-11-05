#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::pallet_prelude::*;
use frame_support::traits::Get;
use frame_system::pallet_prelude::*;
use scale_info::TypeInfo;
use sp_runtime::traits::SaturatedConversion;
use tartarus_primitives::{Balance, DebtId, DebtSignal};

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_mirror::Config + pallet_oubr::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type OubrCount: Get<u32>;
    }

    #[pallet::storage]
    pub type DebtCount<T> = StorageValue<_, DebtId, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        DebtIngested {
            debt_id: DebtId,
            debtor: T::AccountId,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        Unauthorized,
        ZeroOubrCount,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(10_000)]
        pub fn submit_debt_signal(
            origin: OriginFor<T>,
            signal: DebtSignal<T::AccountId>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(who == signal.oracle, Error::<T>::Unauthorized);
            let oubr_count = T::OubrCount::get();
            ensure!(oubr_count > 0, Error::<T>::ZeroOubrCount);

            let debt_id = DebtCount::<T>::get() + 1;
            DebtCount::<T>::put(debt_id);

            let now: u64 = frame_system::Pallet::<T>::block_number().saturated_into();

            // 20% to OUBRs, 80% principal
            let twenty_pct = signal.debt_amount / 5; // 20%
            let principal = signal.debt_amount.saturating_sub(twenty_pct);
            let pot_base = if oubr_count > 0 {
                twenty_pct / (oubr_count as Balance)
            } else {
                0
            };
            let threshold = twenty_pct;

            // mint mirror
            pallet_mirror::Pallet::<T>::mint_mirror(
                debt_id,
                signal.token,
                principal,
                signal.debtor.clone(),
            )?;

            // create oubrs
            pallet_oubr::Pallet::<T>::create_oubrs(
                debt_id,
                signal.dealing_currency,
                pot_base,
                threshold,
                5000,
                oubr_count,
                now.saturating_add(20 * 60),
            );

            Self::deposit_event(Event::DebtIngested {
                debt_id,
                debtor: signal.debtor.clone(),
            });
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn debt_count() -> DebtId {
            DebtCount::<T>::get()
        }
    }
}
