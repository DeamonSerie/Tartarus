#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
use scale_info::TypeInfo;
use sp_runtime::traits::SaturatedConversion;
use tartarus_primitives::{
    Balance, Bundle, BundleId, BundleStatus, CurrencyId, DebtId, OubrStatus,
};

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::BoundedVec;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config:
        frame_system::Config + pallet_mirror::Config + pallet_oubr::Config + pallet_treasury::Config
    {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    }

    #[pallet::storage]
    pub type BundleCount<T> = StorageValue<_, BundleId, ValueQuery>;

    #[pallet::storage]
    pub type Bundles<T: Config> = StorageMap<_, Blake2_128Concat, BundleId, Bundle, OptionQuery>;

    #[pallet::storage]
    pub type BundleBetTotal<T: Config> =
        StorageMap<_, Blake2_128Concat, BundleId, Balance, ValueQuery>;

    #[pallet::storage]
    pub type BundleBets<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        BundleId,
        Blake2_128Concat,
        T::AccountId,
        Balance,
        ValueQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        BundleCreated {
            bundle_id: BundleId,
            debt_id: DebtId,
            currency_id: CurrencyId,
        },
        BundleBet {
            bundle_id: BundleId,
            wallet: T::AccountId,
            amount: Balance,
        },
        BundleFinalized {
            bundle_id: BundleId,
        },
        TreasuryCredited {
            currency_id: CurrencyId,
            amount: Balance,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        BundleNotFound,
        NotInSale,
        SaleEnded,
        SaleNotEnded,
        AlreadyFinalized,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(10_000)]
        pub fn create_bundle(origin: OriginFor<T>, debt_id: DebtId) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            let now: u64 = frame_system::Pallet::<T>::block_number().saturated_into();

            // compute bundle amount
            let principal = pallet_mirror::Pallet::<T>::principal(debt_id);
            let agg = pallet_mirror::Pallet::<T>::aggregate_deposit_value(debt_id);
            let mut bundle_amount = principal.saturating_add(agg);

            let oubr_ids = pallet_oubr::Pallet::<T>::debt_oubrs(debt_id);
            let mut unsettled: u32 = 0;
            for oid in &oubr_ids {
                if let Some(o) = pallet_oubr::Pallet::<T>::get_oubr(*oid) {
                    match o.status {
                        OubrStatus::Settled => {
                            let diff = o.current_price.saturating_sub(o.pot_base);
                            bundle_amount = bundle_amount.saturating_add(diff);
                        }
                        OubrStatus::Burned => {
                            bundle_amount = bundle_amount.saturating_sub(o.pot_base);
                        }
                        OubrStatus::Open => {
                            unsettled = unsettled.saturating_add(1);
                        }
                    }
                }
            }
            if bundle_amount == 0 {
                bundle_amount = 0;
            }

            // price drop for unsettled
            let mut price = bundle_amount;
            for _ in 0..unsettled {
                price = price.saturating_sub(price / 20); // 5%
            }

            let id = BundleCount::<T>::get() + 1;
            let currency_id = id as u32;
            let sale_end = now.saturating_add(40 * 60); // seconds? blocks? treat as time units
            let bundle = Bundle {
                id,
                debt_id,
                currency_id,
                amount: bundle_amount,
                price,
                sale_end,
                unsettled_oubrs: unsettled,
                status: BundleStatus::Sale,
            };
            Bundles::<T>::insert(id, bundle);
            BundleCount::<T>::put(id);
            Self::deposit_event(Event::BundleCreated {
                bundle_id: id,
                debt_id,
                currency_id,
            });
            Ok(())
        }

        #[pallet::weight(10_000)]
        pub fn bet(origin: OriginFor<T>, bundle_id: BundleId, amount: Balance) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let now: u64 = frame_system::Pallet::<T>::block_number().saturated_into();
            let b = Bundles::<T>::get(bundle_id).ok_or(Error::<T>::BundleNotFound)?;
            ensure!(b.status == BundleStatus::Sale, Error::<T>::NotInSale);
            ensure!(now < b.sale_end, Error::<T>::SaleEnded);

            BundleBetTotal::<T>::mutate(bundle_id, |t| *t = t.saturating_add(amount));
            BundleBets::<T>::mutate(bundle_id, &who, |a| *a = a.saturating_add(amount));
            Self::deposit_event(Event::BundleBet {
                bundle_id,
                wallet: who,
                amount,
            });
            Ok(())
        }

        #[pallet::weight(10_000)]
        pub fn finalize(origin: OriginFor<T>, bundle_id: BundleId) -> DispatchResult {
            let _ = origin;
            let now: u64 = frame_system::Pallet::<T>::block_number().saturated_into();
            let mut b = Bundles::<T>::get(bundle_id).ok_or(Error::<T>::BundleNotFound)?;
            ensure!(b.status == BundleStatus::Sale, Error::<T>::AlreadyFinalized);
            ensure!(now >= b.sale_end, Error::<T>::SaleNotEnded);

            let total_bet = BundleBetTotal::<T>::get(bundle_id);
            let unsold = b.amount.saturating_sub(total_bet.min(b.amount));
            if unsold > 0 {
                pallet_treasury::Pallet::<T>::credit(b.currency_id, unsold);
                Self::deposit_event(Event::TreasuryCredited {
                    currency_id: b.currency_id,
                    amount: unsold,
                });
            }
            b.status = BundleStatus::Finalized;
            Bundles::<T>::insert(bundle_id, b);
            Self::deposit_event(Event::BundleFinalized { bundle_id });
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn get_bundle(id: BundleId) -> Option<Bundle> {
            Bundles::<T>::get(id)
        }
    }
}
