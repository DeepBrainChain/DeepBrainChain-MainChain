use crate::{mock::*, Error, MachineGPUOrder, RentOrderDetail, RentOrderId};

use dbc_support::{
    machine_type::MachineStatus,
    rental_type::RentStatus,
    traits::DLCMachineReportStakingTrait,
    verify_slash::{OPPendingSlashInfo, OPSlashReason},
    ONE_DAY,
};
use frame_support::{assert_err, assert_ok, traits::ReservableCurrency};
use once_cell::sync::Lazy;
use online_profile::MachinesInfo;
use rent_machine::ConfirmingOrder;
use sp_core::Pair;
use sp_keyring::AccountKeyring::{Dave, Eve};

const renter_dave: Lazy<sr25519::Public> = Lazy::new(|| sr25519::Public::from(Dave));

const renter_owner: Lazy<sr25519::Public> = Lazy::new(|| sr25519::Public::from(Eve));
const stash: Lazy<sr25519::Public> = Lazy::new(|| sr25519::Public::from(Sr25519Keyring::Ferdie));
const machine_id: Lazy<Vec<u8>> = Lazy::new(|| {
    "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
        .as_bytes()
        .to_vec()
});

#[test]
fn report_dlc_staking_should_works() {
    new_test_ext_after_machine_online().execute_with(|| {
        let msg: Vec<u8> = b"abc".to_vec();
        let eve_sig = Eve.sign(&msg[..]);
        assert_eq!(Eve.public(), sr25519::Public::from(Sr25519Keyring::Eve));

        // 过10个块之后执行租用成功
        run_to_block(10 + 20);

        assert_err!(
            <dlc_machine::Pallet<TestRuntime> as DLCMachineReportStakingTrait>::report_dlc_staking(
                msg.clone(),
                eve_sig.clone(),
                Eve.public(),
                machine_id.clone()
            ),
            "machine not rented"
        );

        assert_ok!(RentMachine::rent_machine(
            RuntimeOrigin::signed(*renter_owner),
            machine_id.clone(),
            4,
            10 * 2880
        ));

        assert_ok!(RentMachine::confirm_rent(RuntimeOrigin::signed(*renter_owner), 0));
        let dlc_machines_online = <dlc_machine::Pallet<TestRuntime>>::dlc_machine_ids_in_staking();
        assert_eq!(dlc_machines_online.contains(&machine_id), false);

        assert_ok!(
            <dlc_machine::Pallet<TestRuntime> as DLCMachineReportStakingTrait>::report_dlc_staking(
                msg,
                eve_sig,
                Eve.public(),
                machine_id.clone()
            )
        );
        let dlc_machines_online = <dlc_machine::Pallet<TestRuntime>>::dlc_machine_ids_in_staking();
        assert_eq!(dlc_machines_online.contains(&machine_id), true)
    })
}

#[test]
fn rent_dlc_machine_should_works() {
    new_test_ext_after_machine_online().execute_with(|| {
        let eve = sp_core::sr25519::Pair::from(Eve);
        let msg: Vec<u8> = b"abc".to_vec();
        let eve_sig = eve.sign(&msg[..]);
        assert_eq!(eve.public(), sr25519::Public::from(Eve));

        // 过10个块之后执行租用成功
        run_to_block(10 + 20);

        assert_err!(
            <dlc_machine::Pallet<TestRuntime> as DLCMachineReportStakingTrait>::report_dlc_staking(
                msg.clone(),
                eve_sig.clone(),
                eve.public(),
                machine_id.clone()
            ),
            "machine not rented"
        );

        assert_ok!(RentMachine::rent_machine(
            RuntimeOrigin::signed(*renter_owner),
            machine_id.clone(),
            4,
            10 * 2880
        ));

        assert_ok!(RentMachine::confirm_rent(RuntimeOrigin::signed(*renter_owner), 0));
        let dlc_machines_online = <dlc_machine::Pallet<TestRuntime>>::dlc_machine_ids_in_staking();
        assert_eq!(dlc_machines_online.contains(&machine_id), false);
        assert_err!(
            RentDlcMachine::rent_dlc_machine(
                RuntimeOrigin::signed(*renter_dave),
                machine_id.clone(),
                4,
                10 * 2880 * 2
            ),
            Error::<TestRuntime>::MachineNotDLCStaking
        );

        assert_ok!(
            <dlc_machine::Pallet<TestRuntime> as DLCMachineReportStakingTrait>::report_dlc_staking(
                msg,
                eve_sig,
                eve.public(),
                machine_id.clone()
            )
        );
        let dlc_machines_online = <dlc_machine::Pallet<TestRuntime>>::dlc_machine_ids_in_staking();
        assert_eq!(dlc_machines_online.contains(&machine_id), true);

        // renter's dlc balance should be 10000000*ONE_DLC before rent dlc machine
        let asset_id = RentDlcMachine::get_dlc_asset_id_parameter();
        assert_eq!(Assets::balance(asset_id.into(), *renter_dave), 10000000 * ONE_DLC);

        //  dlc total_supply should be 10000000*ONE_DLC before rent dlc machine
        assert_eq!(Assets::total_supply(asset_id.into()), 10000000 * ONE_DLC);

        assert_ok!(RentDlcMachine::rent_dlc_machine(
            RuntimeOrigin::signed(*renter_dave),
            machine_id.clone(),
            4,
            10 * 2880 * 2
        ));

        let burn_total = RentDlcMachine::burn_total_amount();
        assert_ne!(burn_total, 0);
        // renter's dlc balance should less than 10000000*ONE_DLC after rent dlc machine
        assert_eq!(Assets::balance(asset_id.into(), *renter_dave), 10000000 * ONE_DLC - burn_total);

        //  dlc total_supply should less than 10000000*ONE_DLC after rent dlc machine
        assert_eq!(Assets::total_supply(asset_id.into()), 10000000 * ONE_DLC - burn_total);

        let rent_order_infos = RentDlcMachine::machine_rent_order(machine_id.clone());
        assert_eq!(rent_order_infos.used_gpu.len(), 4);
        assert_eq!(rent_order_infos.rent_order.len(), 1);
        let rent_dlc_machine_id = rent_order_infos.rent_order[0];
        let rent_info = RentDlcMachine::rent_info(rent_dlc_machine_id.clone()).unwrap();
        assert_eq!(rent_info.rent_status, RentStatus::Renting);
        let rent_duration = rent_info.rent_end.saturating_sub(rent_info.rent_start);
        assert_eq!(rent_duration, 10 * 2880);

        let records = RentDlcMachine::burn_records();
        assert_eq!(records.len(), 1);
        let (burn_amount, burn_at, renter, rent_id) = records[0];
        assert_eq!(burn_amount, burn_total);
        assert_ne!(burn_at, 0);
        assert_eq!(rent_id, rent_dlc_machine_id);
        assert_eq!(renter, Dave.public());

        let dbc_rent_order_infos = RentMachine::machine_rent_order(machine_id.clone());
        let rent_dbc_machine_id = dbc_rent_order_infos.rent_order[0];
        assert_eq!(
            RentDlcMachine::dlc_rent_id_2_parent_dbc_rent_id(rent_dlc_machine_id.clone()).unwrap(),
            rent_dbc_machine_id
        );

        let rent_ids = RentDlcMachine::user_order(Dave.public());
        assert_eq!(rent_ids.len(), 1);

        let rented_gpu_num = RentDlcMachine::dlc_machine_rented_gpu(machine_id.clone());
        assert_eq!(rented_gpu_num, 4);

        run_to_block(30 + 2880 * 10 + 1);

        let rent_ids = RentDlcMachine::user_order(Dave.public());
        assert_eq!(rent_ids.len(), 0);

        assert_eq!(RentDlcMachine::rent_info(rent_dlc_machine_id.clone()).is_none(), true);

        let dlc_rent_order_infos = RentDlcMachine::machine_rent_order(machine_id.clone());
        assert_eq!(dlc_rent_order_infos.rent_order.len(), 0);
        assert_eq!(dlc_rent_order_infos.used_gpu.len(), 0);

        assert_eq!(
            RentDlcMachine::dlc_rent_id_2_parent_dbc_rent_id(rent_dlc_machine_id.clone()).unwrap(),
            rent_dbc_machine_id
        );

        let rented_gpu_num = RentDlcMachine::dlc_machine_rented_gpu(machine_id.clone());
        assert_eq!(rented_gpu_num, 0);
    })
}

#[test]
fn controller_report_offline_when_online_should_work() {
    new_test_ext_after_machine_online().execute_with(|| {
        let controller = sr25519::Public::from(Sr25519Keyring::Eve).into();

        assert_ok!(OnlineProfile::controller_report_offline(
            RuntimeOrigin::signed(controller),
            machine_id.clone()
        ));

        let machine_info = OnlineProfile::machines_info(&*machine_id).unwrap();
        assert_eq!(
            machine_info.machine_status,
            MachineStatus::StakerReportOffline(11, Box::new(MachineStatus::Online))
        );

        // Offline 20 block will result in slash
        run_to_block(20);
        assert_ok!(OnlineProfile::controller_report_online(
            RuntimeOrigin::signed(controller),
            machine_id.clone()
        ));

        assert_eq!(
            OnlineProfile::pending_slash(0),
            Some(OPPendingSlashInfo {
                slash_who: *stash,
                machine_id: machine_id.clone(),
                slash_time: 21,
                slash_amount: 80 * ONE_DBC,
                slash_exec_time: 21 + 2880 * 2,
                reporter: None,
                renters: vec![],
                reward_to_committee: None,
                slash_reason: OPSlashReason::OnlineReportOffline(11)
            })
        );
        // Machine should be online now
        let machine_info = OnlineProfile::machines_info(&*machine_id).unwrap();
        assert_eq!(machine_info.machine_status, MachineStatus::Online);

        // check reserve balance
        assert_eq!(Balances::reserved_balance(*stash), 4080 * ONE_DBC);

        run_to_block(22 + 2880 * 2);
        assert_eq!(OnlineProfile::pending_slash(0), None);
        assert_eq!(Balances::reserved_balance(*stash), 4000 * ONE_DBC);
    })
}

#[test]
fn rent_machine_confirm_expired_should_work() {
    new_test_ext_after_machine_online().execute_with(|| {
        let mut machine_info1 = OnlineProfile::machines_info(&*machine_id).unwrap();

        // Dave rent machine for 10 days
        assert_ok!(RentMachine::rent_machine(
            RuntimeOrigin::signed(*renter_dave),
            machine_id.clone(),
            4,
            10 * 2880
        ));
        let init_rent_order = RentMachine::rent_info(0).unwrap();

        let user_stake = RentMachine::user_total_stake(&*renter_dave);
        assert_eq!(user_stake, 249541666666666666666);

        run_to_block(72);

        {
            // 机器状态
            machine_info1.renters = vec![];
            machine_info1.machine_status = MachineStatus::Online;
            let machine_info2 = OnlineProfile::machines_info(&*machine_id).unwrap();
            assert_eq!(&machine_info1, &machine_info2);

            // 检查租用人质押
            let user_stake = RentMachine::user_total_stake(&*renter_dave);
            assert_eq!(user_stake, 0);

            let empty_rented: Vec<RentOrderId> = vec![];
            assert_eq!(RentMachine::user_order(*renter_dave), empty_rented);

            // RentOrder
            assert_eq!(RentMachine::rent_info(0), None);

            // RentEnding
            assert_eq!(RentMachine::rent_ending(init_rent_order.rent_end), empty_rented);

            // ConfirmingOrder
            assert_eq!(<ConfirmingOrder<TestRuntime>>::contains_key(&0), false);
        }
    })
}

// Case1: after report online, machine status is still rented
#[test]
fn controller_report_offline_when_rented_should_work() {
    new_test_ext_after_machine_online().execute_with(|| {
        // 补充质押
        let mut machine_info = OnlineProfile::machines_info(machine_id.clone()).unwrap();
        let _ = Balances::reserve(&stash, 396000 * ONE_DBC);
        machine_info.stake_amount += 396000 * ONE_DBC;
        MachinesInfo::<TestRuntime>::insert(machine_id.clone(), &machine_info);

        let controller = sr25519::Public::from(Sr25519Keyring::Eve).into();

        assert_ok!(RentMachine::rent_machine(
            RuntimeOrigin::signed(*renter_dave),
            machine_id.clone(),
            4,
            2 * 2880
        ));
        assert_ok!(RentMachine::confirm_rent(RuntimeOrigin::signed(*renter_dave), 0));

        assert_ok!(OnlineProfile::controller_report_offline(
            RuntimeOrigin::signed(controller),
            machine_id.clone()
        ));

        run_to_block(20);
        assert_ok!(OnlineProfile::controller_report_online(
            RuntimeOrigin::signed(controller),
            machine_id.clone()
        ));

        assert_eq!(
            OnlineProfile::pending_slash(0),
            Some(OPPendingSlashInfo {
                slash_who: *stash,
                machine_id: machine_id.clone(),
                slash_time: 21,
                slash_amount: 8000 * ONE_DBC,
                slash_exec_time: 21 + 2880 * 2,
                reporter: None,
                renters: vec![*renter_dave],
                reward_to_committee: None,
                slash_reason: OPSlashReason::RentedReportOffline(11)
            })
        );

        let machine_info = OnlineProfile::machines_info(&*machine_id).unwrap();
        assert_eq!(machine_info.machine_status, MachineStatus::Rented);

        assert_eq!(Balances::reserved_balance(*stash), (400000 + 8000) * ONE_DBC);

        run_to_block(22 + 2880 * 2);
        assert_eq!(OnlineProfile::pending_slash(0), None);
        assert_eq!(Balances::reserved_balance(*stash), 400000 * ONE_DBC);
    })
}

// when machine is rented, controller report offline,
// when machine rent is finished, controller report online
#[test]
fn rented_report_offline_rented_end_report_online() {
    new_test_ext_after_machine_online().execute_with(|| {
        let controller = sr25519::Public::from(Sr25519Keyring::Eve).into();

        // 补充质押 让租金进入算工的余额而不是质押
        let mut machine_info = OnlineProfile::machines_info(machine_id.clone()).unwrap();
        let _ = Balances::reserve(&stash, 396000 * ONE_DBC);
        machine_info.stake_amount += 396000 * ONE_DBC;
        MachinesInfo::<TestRuntime>::insert(machine_id.clone(), &machine_info);

        assert_ok!(RentMachine::rent_machine(
            RuntimeOrigin::signed(*renter_dave),
            machine_id.clone(),
            4,
            1 * 2880
        ));
        assert_ok!(RentMachine::confirm_rent(RuntimeOrigin::signed(*renter_dave), 0));

        // now, rent is 10 block left
        run_to_block(2880);

        let machine_info = OnlineProfile::machines_info(&*machine_id).unwrap();
        assert_eq!(machine_info.machine_status, MachineStatus::Rented);

        assert_ok!(OnlineProfile::controller_report_offline(
            RuntimeOrigin::signed(controller),
            machine_id.clone()
        ));
        run_to_block(3000);

        assert_ok!(OnlineProfile::controller_report_online(
            RuntimeOrigin::signed(controller),
            machine_id.clone()
        ));
        assert_eq!(
            OnlineProfile::pending_slash(0),
            Some(OPPendingSlashInfo {
                slash_who: *stash,
                machine_id: machine_id.clone(),
                slash_time: 3001,
                slash_amount: 16000 * ONE_DBC,
                slash_exec_time: 3001 + 2880 * 2,
                reporter: None,
                renters: vec![],
                reward_to_committee: None,
                slash_reason: OPSlashReason::RentedReportOffline(2881)
            })
        );

        // rent-machine module will do check if rent finished after machine is reonline
        run_to_block(3001);

        let machine_info = OnlineProfile::machines_info(&*machine_id).unwrap();
        assert_eq!(machine_info.machine_status, MachineStatus::Online);
        assert_eq!(machine_info.last_online_height, 3001);
        assert_eq!(machine_info.total_rented_duration, 2880);
        assert_eq!(machine_info.total_rented_times, 1);
    });
}

#[test]
fn controller_report_offline_mutiple_times_should_work() {
    new_test_ext_after_machine_online().execute_with(|| {
        let controller = sr25519::Public::from(Sr25519Keyring::Eve).into();

        assert_ok!(OnlineProfile::controller_report_offline(
            RuntimeOrigin::signed(controller),
            machine_id.clone()
        ));
        assert_ok!(OnlineProfile::controller_report_online(
            RuntimeOrigin::signed(controller),
            machine_id.clone()
        ));

        assert_ok!(OnlineProfile::controller_report_offline(
            RuntimeOrigin::signed(controller),
            machine_id.clone()
        ));
        assert_ok!(OnlineProfile::controller_report_online(
            RuntimeOrigin::signed(controller),
            machine_id.clone()
        ));

        run_to_block(2880 + 20);
        assert_ok!(OnlineProfile::controller_report_offline(
            RuntimeOrigin::signed(controller),
            machine_id.clone()
        ));
        assert_ok!(OnlineProfile::controller_report_online(
            RuntimeOrigin::signed(controller),
            machine_id.clone()
        ));

        // Dave rent machine for 10 days
        assert_ok!(RentMachine::rent_machine(
            RuntimeOrigin::signed(*renter_dave),
            machine_id.clone(),
            4,
            2 * 2880
        ));
        assert_ok!(RentMachine::confirm_rent(RuntimeOrigin::signed(*renter_dave), 0));
        assert_ok!(OnlineProfile::controller_report_offline(
            RuntimeOrigin::signed(controller),
            machine_id.clone()
        ));
        assert_ok!(OnlineProfile::controller_report_online(
            RuntimeOrigin::signed(controller),
            machine_id.clone()
        ));

        run_to_block(2880 * 2 + 20);
        assert_ok!(OnlineProfile::controller_report_offline(
            RuntimeOrigin::signed(controller),
            machine_id.clone()
        ));
        assert_ok!(OnlineProfile::controller_report_online(
            RuntimeOrigin::signed(controller),
            machine_id.clone()
        ));
    })
}

#[test]
fn rent_limit_should_works() {
    new_test_ext_after_machine_online().execute_with(|| {
        // Dave rent machine for 70 days
        assert_ok!(RentMachine::rent_machine(
            RuntimeOrigin::signed(*renter_dave),
            machine_id.clone(),
            4,
            70 * 2880
        ));

        // DBC 价格： 12000 / 10^6 USD
        // 机器价格： 59890 / 1000 * (5000000 / 10^6) USD
        // 需要 DBC的租金:  59890 / 1000 * (5000000 / 10^6) / (12000 / 10^6) * 60 = 1497250
        assert_eq!(RentMachine::user_total_stake(&*renter_dave), 1497250 * ONE_DBC);
        assert_eq!(
            RentMachine::rent_info(0),
            Some(RentOrderDetail {
                machine_id: machine_id.clone(),
                renter: *renter_dave,
                rent_start: 11,
                rent_end: 11 + ONE_DAY as u64 * 60,
                confirm_rent: 0,
                rent_status: RentStatus::WaitingVerifying,
                stake_amount: 1497250 * ONE_DBC,
                gpu_num: 4,
                gpu_index: vec![0, 1, 2, 3],
            })
        );
        assert_eq!(RentMachine::user_order(&*renter_dave), vec![0]);
        assert_eq!(RentMachine::rent_ending((11 + 60 * ONE_DAY) as u64), vec![0]);

        run_to_block(15);
        assert_ok!(RentMachine::confirm_rent(RuntimeOrigin::signed(*renter_dave), 0));
        assert_eq!(
            RentMachine::rent_info(&0),
            Some(RentOrderDetail {
                machine_id: machine_id.clone(),
                renter: *renter_dave,
                rent_start: 11,
                rent_end: 11 + ONE_DAY as u64 * 60,
                confirm_rent: 16,
                rent_status: RentStatus::Renting,
                stake_amount: 0 * ONE_DBC,
                gpu_num: 4,
                gpu_index: vec![0, 1, 2, 3],
            })
        );

        run_to_block(20);
        assert_ok!(RentMachine::relet_machine(RuntimeOrigin::signed(*renter_dave), 0, 1 * 2880));
        assert_eq!(
            RentMachine::rent_info(&0),
            Some(RentOrderDetail {
                machine_id: machine_id.clone(),
                renter: *renter_dave,
                rent_start: 11,
                rent_end: 21 + ONE_DAY as u64 * 60,
                confirm_rent: 16,
                rent_status: RentStatus::Renting,
                stake_amount: 0 * ONE_DBC,
                gpu_num: 4,
                gpu_index: vec![0, 1, 2, 3],
            })
        );

        // 过了一天，续租2天，则只能续租1天
        run_to_block(20 + 2880);
        assert_ok!(RentMachine::relet_machine(RuntimeOrigin::signed(*renter_dave), 0, 2 * 2880));
        assert_eq!(
            RentMachine::rent_info(0),
            Some(RentOrderDetail {
                machine_id: machine_id.clone(),
                renter: *renter_dave,
                rent_start: 11,
                rent_end: 21 + 2880 + ONE_DAY as u64 * 60,
                confirm_rent: 16,
                rent_status: RentStatus::Renting,
                stake_amount: 0 * ONE_DBC,
                gpu_num: 4,
                gpu_index: vec![0, 1, 2, 3],
            })
        );
    })
}

#[test]
fn rent_machine_by_gpu_works() {
    new_test_ext_after_machine_online().execute_with(|| {
        // Dave rent 1 GPU machine for 10 days
        assert_ok!(RentMachine::rent_machine(
            RuntimeOrigin::signed(*renter_dave),
            machine_id.clone(),
            1,
            10 * 2880
        ));

        // - Write: RentOrder, UserOrder, ConfirmingOrder, RentEnding
        {
            assert_eq!(
                RentMachine::rent_info(0),
                Some(RentOrderDetail {
                    machine_id: machine_id.clone(),
                    renter: *renter_dave,
                    rent_start: 11,
                    confirm_rent: 0,
                    rent_end: 10 * 2880 + 11,
                    stake_amount: 62385416666666666666,
                    rent_status: RentStatus::WaitingVerifying,
                    gpu_num: 1,
                    gpu_index: vec![0],
                })
            );

            assert_eq!(RentMachine::user_order(&*renter_dave), vec![0],);

            // 15 min之后需要确认租用
            assert_eq!(RentMachine::confirming_order(11 + 30), vec![0]);

            assert_eq!(RentMachine::rent_ending(10 * 2880 + 11), vec![0]);

            assert_eq!(
                RentMachine::machine_rent_order(&*machine_id),
                MachineGPUOrder { rent_order: vec![0], used_gpu: vec![0] }
            )
        }

        // 过10个块之后执行租用成功
        run_to_block(10 + 20);

        // Dave confirm rent is succeed: should submit confirmation in 30 mins (60 blocks)
        assert_ok!(RentMachine::confirm_rent(RuntimeOrigin::signed(*renter_dave), 0));
    })
}

#[test]
fn get_machine_price_works() {
    // TODO: 测试 get_machine_price
}

// 测试 gen_rentable_gpu
#[test]
fn gen_rentable_gpu_works() {
    let mut machine_rent_order1 = MachineGPUOrder { rent_order: vec![], used_gpu: vec![] };

    assert_eq!(machine_rent_order1.gen_rentable_gpu(1, 4), vec![0]);
    assert_eq!(&machine_rent_order1, &MachineGPUOrder { rent_order: vec![], used_gpu: vec![0] });

    assert_eq!(machine_rent_order1.gen_rentable_gpu(2, 4), vec![1, 2,]);
    assert_eq!(
        &machine_rent_order1,
        &MachineGPUOrder { rent_order: vec![], used_gpu: vec![0, 1, 2] }
    );

    let mut machine_rent_order1 = MachineGPUOrder { rent_order: vec![], used_gpu: vec![1] };
    assert_eq!(machine_rent_order1.gen_rentable_gpu(2, 4), vec![0, 2,]);
    assert_eq!(
        &machine_rent_order1,
        &MachineGPUOrder { rent_order: vec![], used_gpu: vec![0, 1, 2] }
    );
}
