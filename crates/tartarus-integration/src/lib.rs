#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod tests {
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
            Mirror: pallet_mirror,
            Oubr: pallet_oubr,
            Bundle: pallet_bundle,
            Treasury: pallet_treasury,
            Oracle: pallet_oracle,
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

    impl pallet_mirror::Config for TestRuntime {
        type RuntimeEvent = RuntimeEvent;
    }
    impl pallet_oubr::Config for TestRuntime {
        type RuntimeEvent = RuntimeEvent;
    }
    impl pallet_bundle::Config for TestRuntime {
        type RuntimeEvent = RuntimeEvent;
    }
    impl pallet_treasury::Config for TestRuntime {
        type RuntimeEvent = RuntimeEvent;
    }
    impl pallet_oracle::Config for TestRuntime {
        type RuntimeEvent = RuntimeEvent;
        type OubrCount = ConstU32<1>;
    }

    fn new_ext() -> sp_io::TestExternalities {
        let t = frame_system::GenesisConfig::<TestRuntime>::default()
            .build_storage()
            .unwrap();
        sp_io::TestExternalities::new(t)
    }

    #[test]
    fn full_flow_basic() {
        new_ext().execute_with(|| {
            // submit debt signal
            use tartarus_primitives::DebtSignal;
            let signal = DebtSignal {
                oracle: 1u64,
                debtor: 2u64,
                token: 1,
                debt_amount: 1000,
                dealing_currency: 1,
                timestamp: 0,
            };
            pallet_oracle::Pallet::<TestRuntime>::submit_debt_signal(
                frame_system::RawOrigin::Signed(1).into(),
                signal,
            )
            .unwrap();

            // mirror principal 800
            let debt_id = 1u64;
            assert_eq!(
                pallet_mirror::Pallet::<TestRuntime>::principal(debt_id),
                800
            );

            // one oubr created
            let oubrs = pallet_oubr::Pallet::<TestRuntime>::debt_oubrs(debt_id);
            assert_eq!(oubrs.len(), 1);
            let oid = oubrs[0];

            // bet
            // set block number to 1 (window_end ~ 1200+)
            frame_system::Pallet::<TestRuntime>::set_block_number(1);
            pallet_oubr::Pallet::<TestRuntime>::bet(
                frame_system::RawOrigin::Signed(3).into(),
                oid,
                50,
            )
            .unwrap();

            // settle after window: set block number high
            frame_system::Pallet::<TestRuntime>::set_block_number(100000);
            pallet_oubr::Pallet::<TestRuntime>::settle(
                frame_system::RawOrigin::Signed(4).into(),
                oid,
            )
            .unwrap();
            let o = pallet_oubr::Pallet::<TestRuntime>::get_oubr(oid).unwrap();
            assert_eq!(o.status, tartarus_primitives::OubrStatus::Settled);
            assert_eq!(o.winner, Some(3u64));

            // create bundle
            // start mirror for debt
            let supplies = vec![40u128; 10];
            pallet_mirror::Pallet::<TestRuntime>::start_mirror(
                frame_system::RawOrigin::Signed(1).into(),
                debt_id,
                supplies,
            )
            .unwrap();
            pallet_bundle::Pallet::<TestRuntime>::create_bundle(
                frame_system::RawOrigin::Signed(5).into(),
                debt_id,
                0,
            )
            .unwrap();
            let bundles: Vec<_> = (1..=pallet_bundle::Pallet::<TestRuntime>::get_bundle(1)
                .map(|_| 1)
                .unwrap_or(0))
                .collect();
            let b = pallet_bundle::Pallet::<TestRuntime>::get_bundle(1).unwrap();
            assert_eq!(b.debt_id, debt_id);

            // finalize after sale window
            frame_system::Pallet::<TestRuntime>::set_block_number(100000 + 100000);
            pallet_bundle::Pallet::<TestRuntime>::finalize(
                frame_system::RawOrigin::Signed(6).into(),
                1,
            )
            .unwrap();
            let b2 = pallet_bundle::Pallet::<TestRuntime>::get_bundle(1).unwrap();
            assert_eq!(b2.status, tartarus_primitives::BundleStatus::Finalized);
        });
    }

    #[test]
    fn oubr_burn_unbet() {
        new_ext().execute_with(|| {
            let signal = tartarus_primitives::DebtSignal {
                oracle: 1u64,
                debtor: 2u64,
                token: 1,
                debt_amount: 1000,
                dealing_currency: 1,
                timestamp: 0,
            };
            pallet_oracle::Pallet::<TestRuntime>::submit_debt_signal(
                frame_system::RawOrigin::Signed(1).into(),
                signal,
            )
            .unwrap();
            let oubrs = pallet_oubr::Pallet::<TestRuntime>::debt_oubrs(1);
            let oid = oubrs[0];
            frame_system::Pallet::<TestRuntime>::set_block_number(100000);
            pallet_oubr::Pallet::<TestRuntime>::settle(
                frame_system::RawOrigin::Signed(2).into(),
                oid,
            )
            .unwrap();
            let o = pallet_oubr::Pallet::<TestRuntime>::get_oubr(oid).unwrap();
            assert_eq!(o.status, tartarus_primitives::OubrStatus::Burned);
        });
    }

    #[test]
    fn bundle_unsold_to_treasury() {
        new_ext().execute_with(|| {
            let signal = tartarus_primitives::DebtSignal {
                oracle: 1u64,
                debtor: 2u64,
                token: 1,
                debt_amount: 1000,
                dealing_currency: 1,
                timestamp: 0,
            };
            pallet_oracle::Pallet::<TestRuntime>::submit_debt_signal(
                frame_system::RawOrigin::Signed(1).into(),
                signal,
            )
            .unwrap();
            // settle oubr (unbet -> burned)
            let oid = pallet_oubr::Pallet::<TestRuntime>::debt_oubrs(1)[0];
            frame_system::Pallet::<TestRuntime>::set_block_number(100000);
            pallet_oubr::Pallet::<TestRuntime>::settle(
                frame_system::RawOrigin::Signed(2).into(),
                oid,
            )
            .unwrap();
            // start mirror for debt
            let supplies = vec![40u128; 10];
            pallet_mirror::Pallet::<TestRuntime>::start_mirror(
                frame_system::RawOrigin::Signed(1).into(),
                1,
                supplies,
            )
            .unwrap();
            // create bundle
            frame_system::Pallet::<TestRuntime>::set_block_number(100001);
            pallet_bundle::Pallet::<TestRuntime>::create_bundle(
                frame_system::RawOrigin::Signed(3).into(),
                1,
                0,
            )
            .unwrap();
            let b = pallet_bundle::Pallet::<TestRuntime>::get_bundle(1).unwrap();
            let sale_end = b.sale_end;
            // finalize after sale window
            frame_system::Pallet::<TestRuntime>::set_block_number(sale_end + 1);
            pallet_bundle::Pallet::<TestRuntime>::finalize(
                frame_system::RawOrigin::Signed(4).into(),
                1,
            )
            .unwrap();
            // treasury credited with bundle amount
            assert!(pallet_treasury::Pallet::<TestRuntime>::balance(b.currency_id) > 0);
        });
    }

    #[test]
    fn cumulative_winner() {
        new_ext().execute_with(|| {
            let signal = tartarus_primitives::DebtSignal {
                oracle: 1u64,
                debtor: 2u64,
                token: 1,
                debt_amount: 1000,
                dealing_currency: 1,
                timestamp: 0,
            };
            pallet_oracle::Pallet::<TestRuntime>::submit_debt_signal(
                frame_system::RawOrigin::Signed(1).into(),
                signal,
            )
            .unwrap();
            let oid = pallet_oubr::Pallet::<TestRuntime>::debt_oubrs(1)[0];
            frame_system::Pallet::<TestRuntime>::set_block_number(1);
            // wallet 3 bets 10 twice =20; wallet 4 bets 15
            pallet_oubr::Pallet::<TestRuntime>::bet(
                frame_system::RawOrigin::Signed(3).into(),
                oid,
                10,
            )
            .unwrap();
            pallet_oubr::Pallet::<TestRuntime>::bet(
                frame_system::RawOrigin::Signed(4).into(),
                oid,
                15,
            )
            .unwrap();
            pallet_oubr::Pallet::<TestRuntime>::bet(
                frame_system::RawOrigin::Signed(3).into(),
                oid,
                10,
            )
            .unwrap();
            frame_system::Pallet::<TestRuntime>::set_block_number(100000);
            pallet_oubr::Pallet::<TestRuntime>::settle(
                frame_system::RawOrigin::Signed(5).into(),
                oid,
            )
            .unwrap();
            let o = pallet_oubr::Pallet::<TestRuntime>::get_oubr(oid).unwrap();
            assert_eq!(o.winner, Some(3u64));
        });
    }

    #[test]
    fn decay_threshold() {
        new_ext().execute_with(|| {
            let signal = tartarus_primitives::DebtSignal {
                oracle: 1u64,
                debtor: 2u64,
                token: 1,
                debt_amount: 1000,
                dealing_currency: 1,
                timestamp: 0,
            };
            pallet_oracle::Pallet::<TestRuntime>::submit_debt_signal(
                frame_system::RawOrigin::Signed(1).into(),
                signal,
            )
            .unwrap();
            let oid = pallet_oubr::Pallet::<TestRuntime>::debt_oubrs(1)[0];
            frame_system::Pallet::<TestRuntime>::set_block_number(1);
            // threshold = 200. bet 100 -> price 100. bet 200 -> price 300 > threshold -> decay 0.5*200=100 => 200
            pallet_oubr::Pallet::<TestRuntime>::bet(
                frame_system::RawOrigin::Signed(3).into(),
                oid,
                100,
            )
            .unwrap();
            let o = pallet_oubr::Pallet::<TestRuntime>::get_oubr(oid).unwrap();
            assert_eq!(o.current_price, 100);
            pallet_oubr::Pallet::<TestRuntime>::bet(
                frame_system::RawOrigin::Signed(3).into(),
                oid,
                200,
            )
            .unwrap();
            let o = pallet_oubr::Pallet::<TestRuntime>::get_oubr(oid).unwrap();
            assert_eq!(o.current_price, 200);
        });
    }
}
