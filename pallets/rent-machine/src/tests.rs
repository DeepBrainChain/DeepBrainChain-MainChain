use crate::mock::*;
use frame_support::{assert_noop, assert_ok};
use sp_runtime::Perbill;

#[test]
fn rent_machine_should_works() {
    new_test_ext_after_machine_online().execute_with(|| {
        let renter_dave: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Dave).into();

        let _one_committee: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One).into();
        let _pot_two: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Two).into();

        let _controller: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();
        let stash: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();

        // Dave rent machine for 10 days
        assert_ok!(RentMachine::rent_machine(Origin::signed(renter_dave), machine_id.clone(), 10));

        run_to_block(50);

        // Dave confirm rent is succeed: should submit confirmation in 30 mins (60 blocks)
        assert_ok!(RentMachine::confirm_rent(Origin::signed(renter_dave), machine_id.clone()));

        let era_grade_snap = OnlineProfile::eras_stash_points(1).unwrap();
        assert_eq!(era_grade_snap.total, 77881); // 59890 * 4/10000 + 59890 * 0.3 + 59890
        let staker_grade_snap = era_grade_snap.staker_statistic.get(&stash).unwrap();

        assert_eq!(
            staker_grade_snap,
            &online_profile::StashMachineStatistics {
                online_gpu_num: 4,
                inflation: Perbill::from_rational_approximation(4u32, 10000u32),
                machine_total_calc_point: 59890,
                rent_extra_grade: Perbill::from_rational_approximation(30u32, 100u32) * 59890,
            }
        );

        // After rent confirmation, machine grades & reward will change
        let stash_machines = OnlineProfile::stash_machines(&stash);
        assert_eq!(stash_machines.total_rented_gpu, 4);

        // DBC price: {1000 points/ 5_000_000 usd }; 6825 points; 10 eras; DBC price: 12_000 usd
        // So, rent fee: 59890 / 1000 * 5000000 / 12000 * 10 =  249541.6666666667 DBC
        assert_eq!(stash_machines.total_rent_fee, 249541666666666666666);
        // Balance of stash account will increase
        assert_eq!(Balances::free_balance(stash), INIT_BALANCE - 400000 * ONE_DBC + 249541666666666666666);
        // Balance of renter will decrease, Dave is committee so - 20000
        assert_eq!(
            Balances::free_balance(renter_dave),
            INIT_BALANCE - 249541666666666666666 - 10 * ONE_DBC - 20000 * ONE_DBC
        );

        // Dave relet machine
        assert_ok!(RentMachine::relet_machine(Origin::signed(renter_dave), machine_id.clone(), 10));
        assert_eq!(
            RentMachine::rent_order(renter_dave, machine_id.clone()),
            Some(super::RentOrderDetail {
                renter: renter_dave,
                rent_start: 11,
                confirm_rent: 51,
                stake_amount: 0,
                rent_end: (10 + 10) * 2880 + 11,
                rent_status: super::RentStatus::Renting,
            })
        );

        // So balance change should be right
        let stash_machines = OnlineProfile::stash_machines(&stash);
        assert_eq!(stash_machines.total_rent_fee, 249541666666666666666 * 2);
        assert_eq!(Balances::free_balance(stash), INIT_BALANCE + 249541666666666666666 * 2 - 400000 * ONE_DBC);
        assert_eq!(
            Balances::free_balance(renter_dave),
            INIT_BALANCE - 249541666666666666666 * 2 - 10 * ONE_DBC - 20000 * ONE_DBC
        );

        // 21 days later
        run_to_block(60530);
        let era_grade_snap = OnlineProfile::eras_stash_points(21).unwrap();
        assert_eq!(era_grade_snap.total, 59914) // 59890 * 4 / 10000 + 59890
    })
}

#[test]
fn controller_report_offline_when_online_should_work() {
    new_test_ext_after_machine_online().execute_with(|| {
        let controller: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();
        let _stash: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();

        // NOTE: 注意，一天内不能下线两次
        // run_to_block(2880 + 50);

        assert_ok!(OnlineProfile::controller_report_offline(Origin::signed(controller), machine_id.clone()));
    })
}

// Case1: after report online, machine status is still rented
#[test]
fn controller_report_offline_when_rented_should_work1() {
    new_test_ext_after_machine_online().execute_with(|| {
        let _controller: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();
        let _stash: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();
        let _machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();

        run_to_block(50);

        // 机器报告下线，查询存储

        // 机器报告上线，查询存储
    })
}

// Case2: after report online, machine is out of rent duration
#[test]
fn controller_report_offline_when_rented_should_work2() {
    new_test_ext_after_machine_online().execute_with(|| {
        let controller: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();
        let _stash: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();

        run_to_block(50);

        let renter_dave: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Dave).into();

        // Dave rent machine for 10 days
        assert_ok!(RentMachine::rent_machine(Origin::signed(renter_dave), machine_id.clone(), 10));
        run_to_block(51);

        // Dave confirm rent is succeed: should submit confirmation in 30 mins (60 blocks)
        assert_ok!(RentMachine::confirm_rent(Origin::signed(renter_dave), machine_id.clone()));

        assert_noop!(
            OnlineProfile::controller_report_offline(Origin::signed(controller), machine_id.clone()),
            online_profile::Error::<TestRuntime>::CannotOnlineTwiceOneDay
        );

        run_to_block(51 + 2880);
        assert_ok!(OnlineProfile::controller_report_offline(Origin::signed(controller), machine_id.clone()));
    })
}

#[test]
fn controller_report_offline_when_rented_should_work3() {
    new_test_ext_after_machine_online().execute_with(|| {
        let controller: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();
        let _stash: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();

        run_to_block(51 + 2880);

        let renter_dave: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Dave).into();

        // Dave rent machine for 10 days
        assert_ok!(RentMachine::rent_machine(Origin::signed(renter_dave), machine_id.clone(), 10));
        run_to_block(51);

        // Dave confirm rent is succeed: should submit confirmation in 30 mins (60 blocks)
        assert_ok!(RentMachine::confirm_rent(Origin::signed(renter_dave), machine_id.clone()));

        // run_to_block(51 + 2880);
        assert_ok!(OnlineProfile::controller_report_offline(Origin::signed(controller), machine_id.clone()));
    })
}
