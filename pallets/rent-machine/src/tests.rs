use crate::{mock::*, Error, PendingConfirming, RentOrderDetail, BLOCK_PER_DAY};
use frame_support::{assert_noop, assert_ok};
use generic_func::MachineId;
use online_profile::MachineStatus;
use sp_runtime::Perbill;

#[test]
fn rent_machine_should_works() {
    new_test_ext_after_machine_online().execute_with(|| {
        let renter_dave: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Dave).into();
        let stash: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();

        // Dave rent machine for 10 days
        assert_ok!(RentMachine::rent_machine(Origin::signed(renter_dave), machine_id.clone(), 4, 10));

        // 过10个块之后执行租用成功
        run_to_block(10 + 20);

        // Dave confirm rent is succeed: should submit confirmation in 30 mins (60 blocks)
        assert_ok!(RentMachine::confirm_rent(Origin::signed(renter_dave), machine_id.clone()));

        let era_grade_snap = OnlineProfile::eras_stash_points(2);
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
            2 * INIT_BALANCE - 249541666666666666666 - 10 * ONE_DBC - 20000 * ONE_DBC
        );

        // Dave relet machine
        assert_ok!(RentMachine::relet_machine(Origin::signed(renter_dave), machine_id.clone(), 10));
        assert_eq!(
            RentMachine::rent_order(&machine_id),
            super::RentOrderDetail {
                renter: renter_dave,
                rent_start: 11,
                confirm_rent: 31,
                stake_amount: 0,
                rent_end: (10 + 10) * 2880 + 11,
                rent_status: super::RentStatus::Renting,
                gpu_num: 4,
            }
        );

        // So balance change should be right
        let stash_machines = OnlineProfile::stash_machines(&stash);
        assert_eq!(stash_machines.total_rent_fee, 249541666666666666666 * 2);
        assert_eq!(Balances::free_balance(stash), INIT_BALANCE + 249541666666666666666 * 2 - 400000 * ONE_DBC);
        assert_eq!(
            Balances::free_balance(renter_dave),
            2 * INIT_BALANCE - 249541666666666666666 * 2 - 10 * ONE_DBC - 20000 * ONE_DBC
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
                slash_reason: online_profile::OPSlashReason::OnlineReportOffline(11)
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

#[test]
fn rent_machine_confirm_expired_should_work() {
    new_test_ext_after_machine_online().execute_with(|| {
        let renter_dave: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Dave).into();
        let _stash: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();

        let mut machine_info1 = OnlineProfile::machines_info(&machine_id);
        let init_rent_order = RentMachine::rent_order(&machine_id);

        // Dave rent machine for 10 days
        assert_ok!(RentMachine::rent_machine(Origin::signed(renter_dave), machine_id.clone(), 4, 10));

        let user_stake = RentMachine::user_total_stake(&renter_dave);
        assert_eq!(user_stake, 249541666666666666666);

        run_to_block(72);

        {
            // 机器状态
            machine_info1.last_machine_renter = None;
            machine_info1.machine_status = MachineStatus::Online;
            let machine_info2 = OnlineProfile::machines_info(&machine_id);
            assert_eq!(&machine_info1, &machine_info2);

            // 检查租用人质押
            let user_stake = RentMachine::user_total_stake(&renter_dave);
            assert_eq!(user_stake, 0);

            let empty_rented: Vec<MachineId> = vec![];
            assert_eq!(RentMachine::user_rented(&renter_dave), empty_rented);

            // RentOrder
            assert_eq!(RentMachine::rent_order(&machine_id), RentOrderDetail::default());

            // PendingRentEnding
            assert_eq!(RentMachine::pending_rent_ending(init_rent_order.rent_end), empty_rented);

            // PendingConfirming
            assert_eq!(<PendingConfirming<TestRuntime>>::contains_key(&machine_id), false);
        }
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
        assert_ok!(RentMachine::rent_machine(Origin::signed(renter_dave), machine_id.clone(), 4, 2));
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
                slash_reason: online_profile::OPSlashReason::RentedReportOffline(11)
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
        assert_ok!(RentMachine::rent_machine(Origin::signed(renter_dave), machine_id.clone(), 4, 1));
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
                slash_reason: online_profile::OPSlashReason::RentedReportOffline(2881)
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
        assert_ok!(RentMachine::rent_machine(Origin::signed(renter_dave), machine_id.clone(), 4, 2));
        assert_ok!(RentMachine::confirm_rent(Origin::signed(renter_dave), machine_id.clone()));
        assert_ok!(OnlineProfile::controller_report_offline(Origin::signed(controller), machine_id.clone()));
        assert_ok!(OnlineProfile::controller_report_online(Origin::signed(controller), machine_id.clone()));

        run_to_block(2880 * 2 + 20);
        assert_ok!(OnlineProfile::controller_report_offline(Origin::signed(controller), machine_id.clone()));
        assert_ok!(OnlineProfile::controller_report_online(Origin::signed(controller), machine_id.clone()));
    })
}

#[test]
fn rent_limit_should_works() {
    new_test_ext_after_machine_online().execute_with(|| {
        let renter_dave: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Dave).into();
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();

        // Dave rent machine for 70 days
        assert_ok!(RentMachine::rent_machine(Origin::signed(renter_dave), machine_id.clone(), 4, 70));

        // DBC 价格： 12000 / 10^6 USD
        // 机器价格： 59890 / 1000 * (5000000 / 10^6) USD
        // 需要 DBC的租金:  59890 / 1000 * (5000000 / 10^6) / (12000 / 10^6) * 60 = 1497250
        assert_eq!(RentMachine::user_total_stake(&renter_dave), 1497250 * ONE_DBC);
        assert_eq!(
            RentMachine::rent_order(&machine_id),
            super::RentOrderDetail {
                renter: renter_dave,
                rent_start: 11,
                rent_end: 11 + BLOCK_PER_DAY as u64 * 60,
                confirm_rent: 0,
                rent_status: super::RentStatus::WaitingVerifying,
                stake_amount: 1497250 * ONE_DBC,
                gpu_num: 4,
            }
        );
        assert_eq!(RentMachine::user_rented(&renter_dave), vec![machine_id.clone()]);
        assert_eq!(RentMachine::pending_rent_ending((11 + 60 * BLOCK_PER_DAY) as u64), vec![machine_id.clone()]);

        run_to_block(15);
        assert_ok!(RentMachine::confirm_rent(Origin::signed(renter_dave), machine_id.clone()));
        assert_eq!(
            RentMachine::rent_order(&machine_id),
            super::RentOrderDetail {
                renter: renter_dave,
                rent_start: 11,
                rent_end: 11 + BLOCK_PER_DAY as u64 * 60,
                confirm_rent: 16,
                rent_status: super::RentStatus::Renting,
                stake_amount: 0 * ONE_DBC,
                gpu_num: 4,
            }
        );

        run_to_block(20);
        assert_ok!(RentMachine::relet_machine(Origin::signed(renter_dave), machine_id.clone(), 1));
        assert_eq!(
            RentMachine::rent_order(&machine_id),
            super::RentOrderDetail {
                renter: renter_dave,
                rent_start: 11,
                rent_end: 11 + BLOCK_PER_DAY as u64 * 60,
                confirm_rent: 16,
                rent_status: super::RentStatus::Renting,
                stake_amount: 0 * ONE_DBC,
                gpu_num: 4,
            }
        );

        // 过了一天，续租2天，则只能续租1天
        run_to_block(20 + 2880);
        assert_ok!(RentMachine::relet_machine(Origin::signed(renter_dave), machine_id.clone(), 2));
        assert_eq!(
            RentMachine::rent_order(&machine_id),
            super::RentOrderDetail {
                renter: renter_dave,
                rent_start: 11,
                rent_end: 11 + BLOCK_PER_DAY as u64 * (60 + 1),
                confirm_rent: 16,
                rent_status: super::RentStatus::Renting,
                stake_amount: 0 * ONE_DBC,
                gpu_num: 4,
            }
        );
    })
}

#[test]
fn rent_and_relet_by_minutes_works() {
    new_test_ext_after_machine_online().execute_with(|| {
        let renter_dave: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Dave).into();
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();

        assert_eq!(Balances::free_balance(renter_dave), 2 * INIT_BALANCE - 20000 * ONE_DBC);

        // Dave rent machine for 30 minutes
        assert_noop!(
            RentMachine::rent_machine_by_minutes(Origin::signed(renter_dave), machine_id.clone(), 4, 29),
            Error::<TestRuntime>::OnlyHalfHourAllowed
        );
        assert_noop!(
            RentMachine::rent_machine_by_minutes(Origin::signed(renter_dave), machine_id.clone(), 4, 29),
            Error::<TestRuntime>::OnlyHalfHourAllowed
        );
        assert_ok!(RentMachine::rent_machine_by_minutes(Origin::signed(renter_dave), machine_id.clone(), 4, 30));
        {
            // 检查租用人质押
            // DBC price: {1000 points/ 5_000_000 usd }; 6825 points; 1/48 eras; DBC price: 12_000 usd
            // So, rent fee: (59890 / 1000 * 5000000 / 12000) * 1/48 = 24954.166666666668 / 48  = 519.8784722222223 DBC
            let user_stake = RentMachine::user_total_stake(&renter_dave);
            assert_eq!(user_stake, 519878416666666666); // 519.8784166666667 DBC

            assert_eq!(RentMachine::user_rented(&renter_dave), vec![machine_id.clone()]);

            // RentOrder
            assert_eq!(
                RentMachine::rent_order(&machine_id),
                RentOrderDetail {
                    renter: renter_dave,
                    rent_start: 11,
                    confirm_rent: 0,
                    rent_end: 11 + 60, // 租用30min = 60block
                    stake_amount: 519878416666666666,
                    rent_status: crate::RentStatus::WaitingVerifying,
                    gpu_num: 4,
                }
            );

            // PendingRentEnding
            assert_eq!(RentMachine::pending_rent_ending(11 + 60), vec![machine_id.clone()]);

            // PendingConfirming
            assert_eq!(<PendingConfirming<TestRuntime>>::contains_key(&machine_id), true);
        }

        // 检查订单被清理，检查David余额
        run_to_block(10 + 32);
        {
            // 检查租用人质押
            let user_stake = RentMachine::user_total_stake(&renter_dave);
            assert_eq!(user_stake, 0);

            let empty_rented: Vec<MachineId> = vec![];
            assert_eq!(RentMachine::user_rented(&renter_dave), empty_rented);

            // RentOrder
            assert_eq!(RentMachine::rent_order(&machine_id), RentOrderDetail::default());

            // PendingRentEnding
            assert_eq!(RentMachine::pending_rent_ending(11 + 30), empty_rented);

            // PendingConfirming
            assert_eq!(<PendingConfirming<TestRuntime>>::contains_key(&machine_id), false);

            assert_eq!(Balances::free_balance(renter_dave), 2 * INIT_BALANCE - 20000 * ONE_DBC - 10 * ONE_DBC);
        }

        assert_ok!(RentMachine::rent_machine_by_minutes(Origin::signed(renter_dave), machine_id.clone(), 4, 30));

        // Dave confirm rent is succeed: should submit confirmation in 30 mins (60 blocks)
        assert_ok!(RentMachine::confirm_rent(Origin::signed(renter_dave), machine_id.clone()));
        {
            // 检查租用人质押
            let user_stake = RentMachine::user_total_stake(&renter_dave);
            assert_eq!(user_stake, 0);

            let empty_rented: Vec<MachineId> = vec![];
            assert_eq!(RentMachine::user_rented(&renter_dave), vec![machine_id.clone()]);

            // RentOrder
            assert_eq!(
                RentMachine::rent_order(&machine_id),
                RentOrderDetail {
                    renter: renter_dave,
                    rent_start: 43,
                    confirm_rent: 43,
                    rent_end: 43 + 60, // 租用30min = 60block
                    stake_amount: 0,
                    rent_status: crate::RentStatus::Renting,
                    gpu_num: 4,
                }
            );

            // PendingRentEnding
            assert_eq!(RentMachine::pending_rent_ending(11 + 30), empty_rented);

            // PendingConfirming
            assert_eq!(<PendingConfirming<TestRuntime>>::contains_key(&machine_id), false);
        }

        // Dave relet machine
        assert_ok!(RentMachine::relet_machine_by_minutes(Origin::signed(renter_dave), machine_id.clone(), 30));
        {
            // 检查租用人质押
            let user_stake = RentMachine::user_total_stake(&renter_dave);
            assert_eq!(user_stake, 0);

            assert_eq!(RentMachine::user_rented(&renter_dave), vec![machine_id.clone()]);

            // RentOrder
            assert_eq!(
                RentMachine::rent_order(&machine_id),
                RentOrderDetail {
                    renter: renter_dave,
                    rent_start: 43,
                    confirm_rent: 43,
                    rent_end: 43 + 120, // 租用30min = 60block
                    stake_amount: 0,
                    rent_status: crate::RentStatus::Renting,
                    gpu_num: 4,
                }
            );

            // PendingRentEnding
            assert_eq!(RentMachine::pending_rent_ending(43 + 120), vec![machine_id.clone()]);

            // PendingConfirming
            assert_eq!(<PendingConfirming<TestRuntime>>::contains_key(&machine_id), false);

            assert_eq!(
                Balances::free_balance(renter_dave),
                2 * INIT_BALANCE - 20000 * ONE_DBC - 519878416666666666 * 2 - 20 * ONE_DBC
            );
        }
    })
}
