#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
use scale_info::TypeInfo;
use sp_runtime::traits::SaturatedConversion;
use tartarus_primitives::{Balance, Bet, CurrencyId, DebtId, Oubr, OubrId, OubrStatus};

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::BoundedVec;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    }

    #[pallet::storage]
    pub type OubrCount<T> = StorageValue<_, OubrId, ValueQuery>;

    #[pallet::storage]
    pub type Oubrs<T: Config> =
        StorageMap<_, Blake2_128Concat, OubrId, Oubr<T::AccountId>, OptionQuery>;

    #[pallet::storage]
    pub type DebtOubrs<T: Config> =
        StorageMap<_, Blake2_128Concat, DebtId, BoundedVec<OubrId, ConstU32<64>>, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        OubrCreated {
            oubr_id: OubrId,
            debt_id: DebtId,
        },
        OubrBet {
            oubr_id: OubrId,
            wallet: T::AccountId,
            amount: Balance,
        },
        OubrSettled {
            oubr_id: OubrId,
            winner: Option<T::AccountId>,
        },
        OubrBurned {
            oubr_id: OubrId,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        OubrNotFound,
        NotOpen,
        WindowClosed,
        WindowNotEnded,
        NoBets,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(10_000)]
        pub fn bet(origin: OriginFor<T>, oubr_id: OubrId, amount: Balance) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let now: u64 = frame_system::Pallet::<T>::block_number().saturated_into();
            Oubrs::<T>::try_mutate(oubr_id, |maybe| {
                let o = maybe.as_mut().ok_or(Error::<T>::OubrNotFound)?;
                ensure!(o.status == OubrStatus::Open, Error::<T>::NotOpen);
                ensure!(now < o.window_end, Error::<T>::WindowClosed);
                o.bets.push(Bet {
                    wallet: who.clone(),
                    amount,
                    ts: now,
                });
                // price update
                o.current_price = o.current_price.saturating_add(amount);
                if o.current_price > o.threshold {
                    let decay = (o.decay_rate_bp as u128).saturating_mul(amount) / 10000u128;
                    o.current_price = o.current_price.saturating_sub(decay);
                }
                Ok::<(), DispatchError>(())
            })?;
            Self::deposit_event(Event::OubrBet {
                oubr_id,
                wallet: who,
                amount,
            });
            Ok(())
        }

        #[pallet::weight(10_000)]
        pub fn settle(origin: OriginFor<T>, oubr_id: OubrId) -> DispatchResult {
            let _ = origin;
            let now: u64 = frame_system::Pallet::<T>::block_number().saturated_into();
            let mut o = Oubrs::<T>::get(oubr_id).ok_or(Error::<T>::OubrNotFound)?;
            ensure!(o.status == OubrStatus::Open, Error::<T>::NotOpen);
            ensure!(now >= o.window_end, Error::<T>::WindowNotEnded);

            if o.bets.is_empty() {
                o.status = OubrStatus::Burned;
                Oubrs::<T>::insert(oubr_id, o);
                Self::deposit_event(Event::OubrBurned { oubr_id });
                return Ok(());
            }

            // cumulative per wallet
            let mut best: Option<(T::AccountId, Balance)> = None;
            // collect wallets
            let mut wallets: Vec<T::AccountId> = Vec::new();
            for b in &o.bets {
                if !wallets.contains(&b.wallet) {
                    wallets.push(b.wallet.clone());
                }
            }
            for w in wallets {
                let cum: Balance = o
                    .bets
                    .iter()
                    .filter(|b| b.wallet == w)
                    .map(|b| b.amount)
                    .sum();
                match &best {
                    Some((_, amt)) if cum > *amt => best = Some((w, cum)),
                    None => best = Some((w, cum)),
                    _ => {}
                }
            }
            o.winner = best.as_ref().map(|x| x.0.clone());
            o.status = OubrStatus::Settled;
            Oubrs::<T>::insert(oubr_id, o.clone());
            Self::deposit_event(Event::OubrSettled {
                oubr_id,
                winner: o.winner.clone(),
            });
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn create_oubrs(
            debt_id: DebtId,
            dealing_currency: CurrencyId,
            pot_base: Balance,
            threshold: Balance,
            decay_rate_bp: u32,
            count: u32,
            window_end: u64,
        ) -> Vec<OubrId> {
            let mut ids = Vec::new();
            for _ in 0..count {
                let id = OubrCount::<T>::get() + 1;
                let o = Oubr {
                    id,
                    debt_id,
                    dealing_currency,
                    pot_base,
                    current_price: 0,
                    threshold,
                    decay_rate_bp,
                    bets: Vec::new(),
                    window_end,
                    status: OubrStatus::Open,
                    winner: None,
                };
                Oubrs::<T>::insert(id, o);
                OubrCount::<T>::put(id);
                ids.push(id);
                Self::deposit_event(Event::OubrCreated {
                    oubr_id: id,
                    debt_id,
                });
            }
            DebtOubrs::<T>::mutate(debt_id, |v| {
                for id in &ids {
                    let _ = v.try_push(*id);
                }
            });
            ids
        }

        pub fn get_oubr(id: OubrId) -> Option<Oubr<T::AccountId>> {
            Oubrs::<T>::get(id)
        }

        pub fn debt_oubrs(debt_id: DebtId) -> Vec<OubrId> {
            DebtOubrs::<T>::get(debt_id).to_vec()
        }

        pub fn all_settled_or_burned(debt_id: DebtId) -> bool {
            Self::debt_oubrs(debt_id).iter().all(|id| {
                Oubrs::<T>::get(*id)
                    .map(|o| o.status != OubrStatus::Open)
                    .unwrap_or(true)
            })
        }
    }
}
