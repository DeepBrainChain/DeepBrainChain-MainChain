use crate::{
    mock::*, ConfirmingOrder, Error, MachineGPUOrder, RentOrderDetail, RentOrderId, RentStatus,
    WAITING_CONFIRMING_DELAY,
};
use dbc_support::{
    machine_type::MachineStatus,
    verify_slash::{OPPendingSlashInfo, OPSlashReason},
    ONE_DAY, ONE_HOUR, ONE_MINUTE,
};
use frame_support::{assert_noop, assert_ok, traits::ReservableCurrency};
use once_cell::sync::Lazy;
use online_profile::MachinesInfo;
use sp_runtime::Perbill;

const renter_dave: Lazy<sp_core::sr25519::Public> =
    Lazy::new(|| sr25519::Public::from(Sr25519Keyring::Dave));
const stash: Lazy<sp_core::sr25519::Public> =
    Lazy::new(|| sr25519::Public::from(Sr25519Keyring::Ferdie));
const machine_id: Lazy<Vec<u8>> = Lazy::new(|| {
    "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
        .as_bytes()
        .to_vec()
});

#[test]
fn rent_machine_should_works() {
    new_test_ext_after_machine_online().execute_with(|| {
        // Dave rent machine for 10 days
        assert_ok!(RentMachine::rent_machine(
            RuntimeOrigin::signed(*renter_dave),
            machine_id.clone(),
            4,
            10 * ONE_DAY
        ));

        // 过10个块之后执行租用成功
        run_to_block(10 + 20);

        // Dave confirm rent is succeed: should submit confirmation in 30 mins
        assert_ok!(RentMachine::confirm_rent(RuntimeOrigin::signed(*renter_dave), 0));

        let era_grade_snap = OnlineProfile::eras_stash_points(2);
        assert_eq!(era_grade_snap.total, 77881); // 59890 * 4/10000 + 59890 * 0.3 + 59890
        let staker_grade_snap = era_grade_snap.staker_statistic.get(&*stash).unwrap();

        assert_eq!(
            staker_grade_snap,
            &online_profile::StashMachineStatistics {
                online_gpu_num: 4,
                inflation: Perbill::from_rational(4u32, 10000u32),
                machine_total_calc_point: 59890,
                rent_extra_grade: Perbill::from_rational(30u32, 100u32) * 59890,
            }
        );

        // After rent confirmation, machine grades & reward will change
        let stash_machines = OnlineProfile::stash_machines(&*stash);
        assert_eq!(stash_machines.total_rented_gpu, 4);

        // DBC price: {1000 points/ 5_000_000 usd }; 6825 points; 10 eras; DBC price: 12_000 usd
        // So, rent fee: 59890 / 1000 * 5000000 / 12000 * 10 =  249541.6666666667 DBC
        // [Thread B ③] 托管：确认时租金入托管、未入账、未补质押 → 计数 0、质押仍为初始 40000。
        //   （旧模型此处 total_rent_fee=237064…、reserved=40000+237064…；已下移到结算时点，见文末断言。）
        assert_eq!(stash_machines.total_rent_fee, 0);
        assert_eq!(Balances::free_balance(*stash), INIT_BALANCE - 40000 * ONE_DBC);

        assert_eq!(Balances::reserved_balance(*stash), 40000 * ONE_DBC);

        // Balance of renter will decrease, Dave is committee so - 20000
        assert_eq!(
            Balances::free_balance(*renter_dave),
            2 * INIT_BALANCE - 249541666666666666666 - 10 * ONE_DBC - 20000 * ONE_DBC
        );

        // Dave relet machine: order_id == 0
        assert_ok!(RentMachine::relet_machine(
            RuntimeOrigin::signed(*renter_dave),
            0,
            10 * ONE_DAY
        ));
        assert_eq!(
            RentMachine::rent_info(0),
            Some(RentOrderDetail {
                machine_id: machine_id.clone(),
                renter: *renter_dave,
                rent_start: 11,
                confirm_rent: 31,
                rent_end: (10 + 10) * ONE_DAY + 11,
                stake_amount: 0,
                rent_status: RentStatus::Renting,
                gpu_num: 4,
                gpu_index: vec![0, 1, 2, 3],
            })
        );

        // [Thread B ③] 托管：续租费也进托管、仍未入账 → 计数仍 0、质押仍初始 40000（结算才补）。
        let stash_machines = OnlineProfile::stash_machines(&*stash);
        assert_eq!(stash_machines.total_rent_fee, 0);
        assert_eq!(Balances::free_balance(*stash), INIT_BALANCE - 40000 * ONE_DBC);

        assert_eq!(Balances::reserved_balance(*stash), 40000 * ONE_DBC,);

        assert_eq!(
            Balances::free_balance(*renter_dave),
            2 * INIT_BALANCE - 249541666666666666666 * 2 - 10 * ONE_DBC - 20000 * ONE_DBC
        );

        // 21 days later
        run_to_block(50 + 21 * ONE_DAY);
        let era_grade_snap = OnlineProfile::eras_stash_points(21);
        assert_eq!(era_grade_snap.total, 59914); // 59890 * 4 / 10000 + 59890

        // [Thread B ③] 托管收敛：租期(含续租)在 rent_end=20*ONE_DAY+11 已到期全额结算(100% 已用、无罚)。
        //   结算时点补上入账 + 补质押，最终态与旧「确认即付」模型在 1 个最小单位(10^-15 DBC)内一致：
        //   total_rent_fee ≈ 2×237064583333333333333（rent + relet 合并后 settle 时分账一次，
        //   销毁 5% 少 floor 一次 → 比旧模型少 1 base-unit），reserved 补质押到 400000 DBC 目标。
        //   证明托管仅平移入账时点、经济结果守恒。
        let stash_machines = OnlineProfile::stash_machines(&*stash);
        assert_eq!(stash_machines.total_rent_fee, 474129166666666666665);
        assert_eq!(Balances::reserved_balance(*stash), 400000 * ONE_DBC);
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

        // [Thread B ②] 闲置(未租用)机器离线「不罚了」：OnlineReportOffline 罚比 = 0。
        //   之前这里断言 800 DBC 罚金；现在闲置离线仅停奖励(自动)，不扣质押 → 不产生任何 PendingSlash。
        run_to_block(20);
        assert_ok!(OnlineProfile::controller_report_online(
            RuntimeOrigin::signed(controller),
            machine_id.clone()
        ));

        // 无罚：不生成待执行罚单
        assert_eq!(OnlineProfile::pending_slash(0), None);
        // Machine should be online now
        let machine_info = OnlineProfile::machines_info(&*machine_id).unwrap();
        assert_eq!(machine_info.machine_status, MachineStatus::Online);

        // 质押全程未被动过（仍为初始 40000 DBC，没有 +800 的罚金预留）
        assert_eq!(Balances::reserved_balance(*stash), 40000 * ONE_DBC);

        // 再过 2 天执行窗口后依然：无罚单、质押不变
        run_to_block(22 + 2 * ONE_DAY);
        assert_eq!(OnlineProfile::pending_slash(0), None);
        assert_eq!(Balances::reserved_balance(*stash), 40000 * ONE_DBC);
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
            10 * ONE_DAY
        ));
        let init_rent_order = RentMachine::rent_info(0).unwrap();

        let user_stake = RentMachine::user_total_stake(&*renter_dave);
        assert_eq!(user_stake, 249541666666666666666);

        // 30分钟
        run_to_block(11 + 30 * ONE_MINUTE);

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
            2 * ONE_DAY
        ));
        assert_ok!(RentMachine::confirm_rent(RuntimeOrigin::signed(*renter_dave), 0));

        let reserved_before = Balances::reserved_balance(*stash);
        let renter_before = Balances::free_balance(*renter_dave);
        assert!(RentMachine::escrowed_fee(0) > 0, "确认后应已托管");

        // [Thread B ③ · 离线终止] 在租机器控制账户自报离线：新模型 = 终止租约 + 结算托管 + **不罚 stake**。
        //   刚开租即离线(elapsed≈0) → 已用≈0 → 租客拿回几乎全部预付租金、矿工≈0、penalty≈0。
        assert_ok!(OnlineProfile::controller_report_offline(
            RuntimeOrigin::signed(controller),
            machine_id.clone()
        ));

        // 租约已终止、托管已结清、无 stake 罚单（旧模型此处产生 8720 DBC 罚；③ 归 0）
        assert_eq!(RentMachine::rent_info(0), None, "离线 → 租约终止");
        assert_eq!(RentMachine::escrowed_fee(0), 0, "托管已结清");
        assert_eq!(OnlineProfile::pending_slash(0), None, "rented-offline 不罚 stake");
        // 早退退款：租客自由余额回升
        assert!(
            Balances::free_balance(*renter_dave) > renter_before,
            "早退应退还未用预付租金给租客"
        );
        // 质押 bond 全程未被动（无 8720 罚金预留、过额质押无补质押）
        assert_eq!(Balances::reserved_balance(*stash), reserved_before, "stake bond 不碰");

        run_to_block(20);
        assert_ok!(OnlineProfile::controller_report_online(
            RuntimeOrigin::signed(controller),
            machine_id.clone()
        ));

        // 租约已终止 → 恢复上线为 Online（非 Rented），仍无罚、质押不变
        let machine_info = OnlineProfile::machines_info(&*machine_id).unwrap();
        assert_eq!(machine_info.machine_status, MachineStatus::Online);
        assert_eq!(OnlineProfile::pending_slash(0), None);
        assert_eq!(Balances::reserved_balance(*stash), reserved_before);
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
            1 * ONE_DAY
        ));
        assert_ok!(RentMachine::confirm_rent(RuntimeOrigin::signed(*renter_dave), 0));

        // 推进到接近满租（1 天租期，rent_end = 1 + ONE_DAY）
        run_to_block(ONE_DAY);

        let machine_info = OnlineProfile::machines_info(&*machine_id).unwrap();
        assert_eq!(machine_info.machine_status, MachineStatus::Rented);

        let reserved_before = Balances::reserved_balance(*stash);
        let renter_before = Balances::free_balance(*renter_dave);

        // [Thread B ③ · 离线终止] 临近满租时离线：已用≈整日，但 ≤24h 罚金封顶 = 整日租金(1天租期) →
        //   矿工净收≈0、租客拿回≈整日租金(除 5% 销毁)作为离线补偿、**不罚 stake**。租约当场终止。
        assert_ok!(OnlineProfile::controller_report_offline(
            RuntimeOrigin::signed(controller),
            machine_id.clone()
        ));
        assert_eq!(RentMachine::rent_info(0), None, "离线 → 租约终止");
        assert_eq!(RentMachine::escrowed_fee(0), 0, "托管已结清");
        assert_eq!(OnlineProfile::pending_slash(0), None, "rented-offline 不罚 stake（旧模型此处 17440 DBC）");
        // 离线补偿：租客自由余额回升（≈整日租金的离线罚金）
        assert!(
            Balances::free_balance(*renter_dave) > renter_before,
            "离线应补偿租客(≤24h 租金)"
        );
        assert_eq!(Balances::reserved_balance(*stash), reserved_before, "stake bond 不碰");

        run_to_block(11 + ONE_DAY + ONE_HOUR);
        assert_ok!(OnlineProfile::controller_report_online(
            RuntimeOrigin::signed(controller),
            machine_id.clone()
        ));

        // 租约已终止 → 恢复上线为 Online，无罚
        let machine_info = OnlineProfile::machines_info(&*machine_id).unwrap();
        assert_eq!(machine_info.machine_status, MachineStatus::Online);
        assert_eq!(OnlineProfile::pending_slash(0), None);
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

        run_to_block(20 + ONE_DAY);
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
            2 * ONE_DAY
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

        run_to_block(20 + 2 * ONE_DAY);
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
            70 * ONE_DAY
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
                rent_end: 11 + 60 * ONE_DAY,
                confirm_rent: 0,
                rent_status: RentStatus::WaitingVerifying,
                stake_amount: 1497250 * ONE_DBC,
                gpu_num: 4,
                gpu_index: vec![0, 1, 2, 3],
            })
        );
        assert_eq!(RentMachine::user_order(&*renter_dave), vec![0]);
        assert_eq!(RentMachine::rent_ending(11 + 60 * ONE_DAY), vec![0]);

        run_to_block(15);
        assert_ok!(RentMachine::confirm_rent(RuntimeOrigin::signed(*renter_dave), 0));
        assert_eq!(
            RentMachine::rent_info(&0),
            Some(RentOrderDetail {
                machine_id: machine_id.clone(),
                renter: *renter_dave,
                rent_start: 11,
                rent_end: 11 + 60 * ONE_DAY,
                confirm_rent: 16,
                rent_status: RentStatus::Renting,
                stake_amount: 0 * ONE_DBC,
                gpu_num: 4,
                gpu_index: vec![0, 1, 2, 3],
            })
        );

        run_to_block(20);
        assert_ok!(RentMachine::relet_machine(RuntimeOrigin::signed(*renter_dave), 0, 1 * ONE_DAY));
        assert_eq!(
            RentMachine::rent_info(&0),
            Some(RentOrderDetail {
                machine_id: machine_id.clone(),
                renter: *renter_dave,
                rent_start: 11,
                rent_end: 21 + 60 * ONE_DAY,
                confirm_rent: 16,
                rent_status: RentStatus::Renting,
                stake_amount: 0 * ONE_DBC,
                gpu_num: 4,
                gpu_index: vec![0, 1, 2, 3],
            })
        );

        // 过了一天，续租2天，则只能续租1天
        run_to_block(20 + ONE_DAY);
        assert_ok!(RentMachine::relet_machine(RuntimeOrigin::signed(*renter_dave), 0, 2 * ONE_DAY));
        assert_eq!(
            RentMachine::rent_info(0),
            Some(RentOrderDetail {
                machine_id: machine_id.clone(),
                renter: *renter_dave,
                rent_start: 11,
                rent_end: 21 + ONE_DAY + 60 * ONE_DAY,
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
fn rent_and_relet_by_minutes_works() {
    new_test_ext_after_machine_online().execute_with(|| {
        assert_eq!(Balances::free_balance(*renter_dave), 2 * INIT_BALANCE - 20000 * ONE_DBC);

        // Dave rent machine for 30 minutes
        assert_noop!(
            RentMachine::rent_machine(
                RuntimeOrigin::signed(*renter_dave),
                machine_id.clone(),
                4,
                29 * ONE_MINUTE
            ),
            Error::<TestRuntime>::OnlyHalfHourAllowed
        );
        assert_noop!(
            RentMachine::rent_machine(
                RuntimeOrigin::signed(*renter_dave),
                machine_id.clone(),
                4,
                29 * ONE_MINUTE
            ),
            Error::<TestRuntime>::OnlyHalfHourAllowed
        );
        assert_ok!(RentMachine::rent_machine(
            RuntimeOrigin::signed(*renter_dave),
            machine_id.clone(),
            4,
            30 * ONE_MINUTE
        ));
        {
            // 检查租用人质押
            // DBC price: {1000 points/ 5_000_000 usd }; 6825 points; 1/48 eras; DBC price: 12_000
            // usd So, rent fee: (59890 / 1000 * 5000000 / 12000) * 1/48 =
            // 24954.166666666668 / 48  = 519.8784722222223 DBC
            let user_stake = RentMachine::user_total_stake(&*renter_dave);
            assert_eq!(user_stake, 519878416666666666); // 519.8784166666667 DBC

            assert_eq!(RentMachine::user_order(&*renter_dave), vec![0]);

            // RentOrder
            assert_eq!(
                RentMachine::rent_info(0),
                Some(RentOrderDetail {
                    machine_id: machine_id.clone(),
                    renter: *renter_dave,
                    rent_start: 11,
                    confirm_rent: 0,
                    rent_end: 11 + 30 * ONE_MINUTE, // 租用30min
                    stake_amount: 519878416666666666,
                    rent_status: crate::RentStatus::WaitingVerifying,
                    gpu_num: 4,
                    gpu_index: vec![0, 1, 2, 3],
                })
            );

            // RentEnding
            assert_eq!(RentMachine::rent_ending(11 + 30 * ONE_MINUTE), vec![0]);

            // ConfirmingOrder
            assert_eq!(
                <ConfirmingOrder<TestRuntime>>::contains_key(11 + WAITING_CONFIRMING_DELAY),
                true
            );
        }

        // 检查订单被清理，检查David余额
        run_to_block(12 + 30 * ONE_MINUTE);
        {
            // 检查租用人质押
            let user_stake = RentMachine::user_total_stake(&*renter_dave);
            assert_eq!(user_stake, 0);

            let empty_rented: Vec<RentOrderId> = vec![];
            assert_eq!(RentMachine::user_order(&*renter_dave), empty_rented);

            // RentOrder
            assert_eq!(RentMachine::rent_info(0), None);

            // RentEnding
            assert_eq!(RentMachine::rent_ending(11 + 30), empty_rented);

            // ConfirmingOrder
            assert_eq!(<ConfirmingOrder<TestRuntime>>::contains_key(0), false);

            assert_eq!(
                RentMachine::machine_rent_order(&*machine_id),
                MachineGPUOrder { rent_order: vec![], used_gpu: vec![] }
            );

            assert_eq!(
                Balances::free_balance(*renter_dave),
                2 * INIT_BALANCE - 20000 * ONE_DBC - 10 * ONE_DBC
            );
        }

        assert_ok!(RentMachine::rent_machine(
            RuntimeOrigin::signed(*renter_dave),
            machine_id.clone(),
            4,
            30 * ONE_MINUTE
        ));
        {
            assert_eq!(
                RentMachine::rent_info(1),
                Some(RentOrderDetail {
                    machine_id: machine_id.clone(),
                    renter: *renter_dave,
                    rent_start: 13 + 30 * ONE_MINUTE,
                    confirm_rent: 0,
                    rent_end: 13 + 30 * ONE_MINUTE + 30 * ONE_MINUTE, // 租用30min
                    stake_amount: 519878416666666666,
                    rent_status: crate::RentStatus::WaitingVerifying,
                    gpu_num: 4,
                    gpu_index: vec![0, 1, 2, 3],
                })
            );
        }

        // Dave confirm rent is succeed: should submit confirmation in 30 mins
        assert_ok!(RentMachine::confirm_rent(RuntimeOrigin::signed(*renter_dave), 1));
        {
            // 检查租用人质押
            let user_stake = RentMachine::user_total_stake(&*renter_dave);
            assert_eq!(user_stake, 0);

            let empty_rented: Vec<RentOrderId> = vec![];
            assert_eq!(RentMachine::user_order(&*renter_dave), vec![1]);

            // RentOrder
            assert_eq!(
                RentMachine::rent_info(1),
                Some(RentOrderDetail {
                    machine_id: machine_id.clone(),
                    renter: *renter_dave,
                    rent_start: 13 + 30 * ONE_MINUTE,
                    confirm_rent: 13 + 30 * ONE_MINUTE,
                    rent_end: 13 + 30 * ONE_MINUTE + 30 * ONE_MINUTE, // 租用30min
                    stake_amount: 0,
                    rent_status: crate::RentStatus::Renting,
                    gpu_num: 4,
                    gpu_index: vec![0, 1, 2, 3],
                })
            );

            // RentEnding
            assert_eq!(RentMachine::rent_ending(11 + 30), empty_rented);

            // ConfirmingOrder
            assert_eq!(<ConfirmingOrder<TestRuntime>>::contains_key(&0), false);
        }

        // Dave relet machine
        assert_ok!(RentMachine::relet_machine(
            RuntimeOrigin::signed(*renter_dave),
            1,
            30 * ONE_MINUTE
        ));
        {
            // 检查租用人质押
            let user_stake = RentMachine::user_total_stake(&*renter_dave);
            assert_eq!(user_stake, 0);

            assert_eq!(RentMachine::user_order(&*renter_dave), vec![1]);

            // RentOrder
            assert_eq!(
                RentMachine::rent_info(1),
                Some(RentOrderDetail {
                    machine_id: machine_id.clone(),
                    renter: *renter_dave,
                    rent_start: 13 + 30 * ONE_MINUTE,
                    confirm_rent: 13 + 30 * ONE_MINUTE,
                    rent_end: 13 + 30 * ONE_MINUTE + 60 * ONE_MINUTE, // 租用60min
                    stake_amount: 0,
                    rent_status: crate::RentStatus::Renting,
                    gpu_num: 4,
                    gpu_index: vec![0, 1, 2, 3],
                })
            );

            // RentEnding
            assert_eq!(RentMachine::rent_ending(13 + 30 * ONE_MINUTE + 60 * ONE_MINUTE), vec![1]);

            // ConfirmingOrder
            assert_eq!(<ConfirmingOrder<TestRuntime>>::contains_key(0), false);

            assert_eq!(
                Balances::free_balance(*renter_dave),
                2 * INIT_BALANCE - 20000 * ONE_DBC - 519878416666666666 * 2 - 20 * ONE_DBC
            );
        }
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
            10 * ONE_DAY
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
                    rent_end: 10 * ONE_DAY + 11,
                    stake_amount: 62385416666666666666,
                    rent_status: RentStatus::WaitingVerifying,
                    gpu_num: 1,
                    gpu_index: vec![0],
                })
            );

            assert_eq!(RentMachine::user_order(&*renter_dave), vec![0],);

            // 15 min之后需要确认租用
            assert_eq!(RentMachine::confirming_order(11 + WAITING_CONFIRMING_DELAY), vec![0]);

            assert_eq!(RentMachine::rent_ending(10 * ONE_DAY + 11), vec![0]);

            assert_eq!(
                RentMachine::machine_rent_order(&*machine_id),
                MachineGPUOrder { rent_order: vec![0], used_gpu: vec![0] }
            )
        }

        // 过10个块之后执行租用成功
        run_to_block(10 + 20);

        // Dave confirm rent is succeed: should submit confirmation in 30 mins
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

// [审计修 H3/round2 回归] 拒绝 0 卡租用（否则可零成本堆空订单放大 on_finalize 结算量）。
#[test]
fn rent_machine_rejects_zero_gpu() {
    new_test_ext_after_machine_online().execute_with(|| {
        assert_noop!(
            RentMachine::rent_machine(
                RuntimeOrigin::signed(*renter_dave),
                machine_id.clone(),
                0,
                2 * ONE_DAY
            ),
            Error::<TestRuntime>::InvalidRentGpuNum
        );
    })
}
