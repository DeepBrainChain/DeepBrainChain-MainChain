use crate::mock::*;
use frame_support::assert_ok;
use online_profile::MachineStatus;
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

        let era_grade_snap = OnlineProfile::eras_stash_points(1);
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
            RentMachine::rent_order(&machine_id),
            super::RentOrderDetail {
                renter: renter_dave,
                rent_start: 11,
                confirm_rent: 51,
                stake_amount: 0,
                rent_end: (10 + 10) * 2880 + 11,
                rent_status: super::RentStatus::Renting,
            }
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
        let era_grade_snap = OnlineProfile::eras_stash_points(21);
        assert_eq!(era_grade_snap.total, 59914) // 59890 * 4 / 10000 + 59890
    })
}

#[test]
fn controller_report_offline_when_online_should_work() {
    new_test_ext_after_machine_online().execute_with(|| {
        let controller: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();
        let stash: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();

        assert_ok!(OnlineProfile::controller_report_offline(Origin::signed(controller), machine_id.clone()));

        let machine_info = OnlineProfile::machines_info(&machine_id);
        assert_eq!(
            machine_info.machine_status,
            online_profile::MachineStatus::StakerReportOffline(11, Box::new(online_profile::MachineStatus::Online))
        );

        // Offline 20 block will result in slash
        run_to_block(20);
        assert_ok!(OnlineProfile::controller_report_online(Origin::signed(controller), machine_id.clone()));

        assert_eq!(
            OnlineProfile::pending_slash(0),
            online_profile::OPPendingSlashInfo {
                slash_who: stash,
                machine_id: machine_id.clone(),
                slash_time: 21,
                slash_amount: 8000 * ONE_DBC,
                slash_exec_time: 21 + 2880 * 2,
                reward_to_reporter: None,
                reward_to_committee: None,
                slash_reason: online_profile::OPSlashReason::OnlineReportOffline(10)
            }
        );
        // Machine should be online now
        let machine_info = OnlineProfile::machines_info(&machine_id);
        assert_eq!(machine_info.machine_status, online_profile::MachineStatus::Online);

        // check reserve balance
        assert_eq!(Balances::reserved_balance(stash), 408000 * ONE_DBC);

        run_to_block(22 + 2880 * 2);
        assert_eq!(OnlineProfile::pending_slash(0), online_profile::OPPendingSlashInfo { ..Default::default() });
        assert_eq!(Balances::reserved_balance(stash), 400000 * ONE_DBC);
    })
}

// Case1: after report online, machine status is still rented
#[test]
fn controller_report_offline_when_rented_should_work() {
    new_test_ext_after_machine_online().execute_with(|| {
        let controller: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();
        let stash: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();

        let renter_dave: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Dave).into();
        assert_ok!(RentMachine::rent_machine(Origin::signed(renter_dave), machine_id.clone(), 2));
        assert_ok!(RentMachine::confirm_rent(Origin::signed(renter_dave), machine_id.clone()));

        assert_ok!(OnlineProfile::controller_report_offline(Origin::signed(controller), machine_id.clone()));

        run_to_block(20);
        assert_ok!(OnlineProfile::controller_report_online(Origin::signed(controller), machine_id.clone()));

        assert_eq!(
            OnlineProfile::pending_slash(0),
            online_profile::OPPendingSlashInfo {
                slash_who: stash,
                machine_id: machine_id.clone(),
                slash_time: 21,
                slash_amount: 8000 * ONE_DBC,
                slash_exec_time: 21 + 2880 * 2,
                reward_to_reporter: None,
                reward_to_committee: None,
                slash_reason: online_profile::OPSlashReason::RentedReportOffline(10)
            }
        );

        let machine_info = OnlineProfile::machines_info(&machine_id);
        assert_eq!(machine_info.machine_status, online_profile::MachineStatus::Rented);

        assert_eq!(Balances::reserved_balance(stash), 408000 * ONE_DBC);

        run_to_block(22 + 2880 * 2);
        assert_eq!(OnlineProfile::pending_slash(0), online_profile::OPPendingSlashInfo { ..Default::default() });
        assert_eq!(Balances::reserved_balance(stash), 400000 * ONE_DBC);
    })
}

// when machine is rented, controller report offline,
// when machine rent is finished, controller report online
#[test]
fn rented_report_offline_rented_end_report_online() {
    new_test_ext_after_machine_online().execute_with(|| {
        let controller: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();
        let stash: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();

        let renter_dave: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Dave).into();
        assert_ok!(RentMachine::rent_machine(Origin::signed(renter_dave), machine_id.clone(), 1));
        assert_ok!(RentMachine::confirm_rent(Origin::signed(renter_dave), machine_id.clone()));

        // now, rent is 10 block left
        run_to_block(2880);

        let machine_info = OnlineProfile::machines_info(&machine_id);
        assert_eq!(machine_info.machine_status, online_profile::MachineStatus::Rented);

        assert_ok!(OnlineProfile::controller_report_offline(Origin::signed(controller), machine_id.clone()));
        run_to_block(3000);

        assert_ok!(OnlineProfile::controller_report_online(Origin::signed(controller), machine_id.clone()));
        assert_eq!(
            OnlineProfile::pending_slash(0),
            online_profile::OPPendingSlashInfo {
                slash_who: stash,
                machine_id: machine_id.clone(),
                slash_time: 3001,
                slash_amount: 16000 * ONE_DBC,
                slash_exec_time: 3001 + 2880 * 2,
                reward_to_reporter: None,
                reward_to_committee: None,
                slash_reason: online_profile::OPSlashReason::RentedReportOffline(120)
            }
        );

        // rent-machine module will do check if rent finished after machine is reonline
        run_to_block(3001);

        let machine_info = OnlineProfile::machines_info(&machine_id);
        assert_eq!(machine_info.machine_status, online_profile::MachineStatus::Online);
        assert_eq!(machine_info.last_online_height, 3001);
        assert_eq!(machine_info.total_rented_duration, 1);
        assert_eq!(machine_info.total_rented_times, 1);
    });
}

#[test]
fn controller_report_offline_mutiple_times_should_work() {
    new_test_ext_after_machine_online().execute_with(|| {
        let controller: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();
        let _stash: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();

        let renter_dave: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Dave).into();

        assert_ok!(OnlineProfile::controller_report_offline(Origin::signed(controller), machine_id.clone()));
        assert_ok!(OnlineProfile::controller_report_online(Origin::signed(controller), machine_id.clone()));

        assert_ok!(OnlineProfile::controller_report_offline(Origin::signed(controller), machine_id.clone()));
        assert_ok!(OnlineProfile::controller_report_online(Origin::signed(controller), machine_id.clone()));

        run_to_block(2880 + 20);
        assert_ok!(OnlineProfile::controller_report_offline(Origin::signed(controller), machine_id.clone()));
        assert_ok!(OnlineProfile::controller_report_online(Origin::signed(controller), machine_id.clone()));

        // Dave rent machine for 10 days
        assert_ok!(RentMachine::rent_machine(Origin::signed(renter_dave), machine_id.clone(), 2));
        assert_ok!(RentMachine::confirm_rent(Origin::signed(renter_dave), machine_id.clone()));
        assert_ok!(OnlineProfile::controller_report_offline(Origin::signed(controller), machine_id.clone()));
        assert_ok!(OnlineProfile::controller_report_online(Origin::signed(controller), machine_id.clone()));

        run_to_block(2880 * 2 + 20);
        assert_ok!(OnlineProfile::controller_report_offline(Origin::signed(controller), machine_id.clone()));
        assert_ok!(OnlineProfile::controller_report_online(Origin::signed(controller), machine_id.clone()));
    })
}
