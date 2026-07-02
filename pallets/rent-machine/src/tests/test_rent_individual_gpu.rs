use super::super::mock::*;
use crate::{
    ConfirmingOrder, MachineGPUOrder, RentInfo, RentOrderDetail, RentOrderId, RentStatus,
    WAITING_CONFIRMING_DELAY,
};
use dbc_support::{
    live_machine::LiveMachine, machine_type::MachineStatus, verify_online::StashMachine, ONE_DAY,
    ONE_HOUR,
};
use frame_support::{assert_ok, traits::ReservableCurrency};
use online_profile::{EraStashPoints, MachinesInfo, SysInfoDetail};

#[test]
fn report_individual_gpu() {
    // 一个机器被两个人进行租用，其中一个进行举报，举报成功，将另一个进行下架。
    new_test_ext_after_machine_online().execute_with(|| {
        let stash = sr25519::Public::from(Sr25519Keyring::Ferdie);

        let renter1 = sr25519::Public::from(Sr25519Keyring::Alice);
        let renter2 = sr25519::Public::from(Sr25519Keyring::Bob);

        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();

        // 补充质押 让租金进入算工的余额而不是质押
        let mut machine_info = OnlineProfile::machines_info(&machine_id).unwrap();
        let _ = Balances::reserve(&stash, 396000 * ONE_DBC);
        machine_info.stake_amount += 396000 * ONE_DBC;
        MachinesInfo::<TestRuntime>::insert(&machine_id, &machine_info);

        // 两人各租用1台机器
        assert_ok!(RentMachine::rent_machine(
            RuntimeOrigin::signed(renter1),
            machine_id.clone(),
            2,
            1 * ONE_DAY
        ));
        // 检查 renter1 状态
        {
            // rent_machine:
            // Balance: 支付10 DBC; UserTotalStake; NextRentId; RentInfo; UserOrder;
            // RentEnding; ConfirmingOrder; MachineRentOrder;
            //

            // DBC 价格：1 DBC = 12_000 / 10^6 USD
            // 机器价格：59890 / 1000 * (50_000_000/10^6) USD = 2994.5
            // 每天需要租金： 机器价格/DBC价格
            // 1000 point -> 50_000_000; 59890 -> 748625 DBC，因此两个GPU: 374312.5 DBC
            assert_eq!(RentMachine::user_total_stake(renter1), 12477083333333333333);
            assert_eq!(RentMachine::next_rent_id(), 1);
            assert_eq!(
                RentMachine::rent_info(0),
                Some(RentOrderDetail {
                    machine_id: machine_id.clone(),
                    renter: renter1,
                    rent_start: 11,
                    confirm_rent: 0,
                    rent_end: 11 + ONE_DAY,
                    stake_amount: 12477083333333333333,
                    rent_status: RentStatus::WaitingVerifying,
                    gpu_num: 2,
                    gpu_index: vec![0, 1],
                })
            );
            assert_eq!(RentMachine::user_order(&renter1), vec![0]);
            assert_eq!(RentMachine::rent_ending(11 + ONE_DAY), vec![0]);
            assert_eq!(RentMachine::confirming_order(11 + WAITING_CONFIRMING_DELAY), vec![0]);
            assert_eq!(
                RentMachine::machine_rent_order(&machine_id),
                MachineGPUOrder { rent_order: vec![0], used_gpu: vec![0, 1] }
            );

            // online_profile:
            // machine_info; machine_rented_gpu;
            assert_eq!(OnlineProfile::machine_rented_gpu(&machine_id), 2);
            let machine_info = OnlineProfile::machines_info(&machine_id).unwrap();
            assert_eq!(machine_info.machine_status, MachineStatus::Rented);
        }

        // 检查renter2 状态，应该与1一致
        assert_ok!(RentMachine::rent_machine(
            RuntimeOrigin::signed(renter2),
            machine_id.clone(),
            2,
            1 * ONE_DAY
        ));
        // 检查状态
        {
            // rent_machine:
            // Balance: 支付10 DBC; UserTotalStake; NextRentId; RentInfo; UserOrder;
            // RentEnding; ConfirmingOrder; MachineRentOrder;

            assert_eq!(RentMachine::user_total_stake(renter1), 12477083333333333333);
            assert_eq!(RentMachine::next_rent_id(), 2);
            assert_eq!(
                RentMachine::rent_info(1),
                Some(RentOrderDetail {
                    machine_id: machine_id.clone(),
                    renter: renter2,
                    rent_start: 11,
                    confirm_rent: 0,
                    rent_end: 11 + ONE_DAY,
                    stake_amount: 12477083333333333333,
                    rent_status: RentStatus::WaitingVerifying,
                    gpu_num: 2,
                    gpu_index: vec![2, 3],
                })
            );
            assert_eq!(RentMachine::user_order(&renter1), vec![0]);
            assert_eq!(RentMachine::rent_ending(11 + ONE_DAY), vec![0, 1]);
            assert_eq!(RentMachine::confirming_order(11 + WAITING_CONFIRMING_DELAY), vec![0, 1]);
            assert_eq!(
                RentMachine::machine_rent_order(&machine_id),
                MachineGPUOrder { rent_order: vec![0, 1], used_gpu: vec![0, 1, 2, 3] }
            );

            // online_profile:
            // machine_info; machine_rented_gpu;
            assert_eq!(OnlineProfile::machine_rented_gpu(&machine_id), 4);
            let machine_info = OnlineProfile::machines_info(&machine_id).unwrap();
            assert_eq!(machine_info.machine_status, MachineStatus::Rented);
        }

        // 两个订单分别进行确认租用
        assert_ok!(RentMachine::confirm_rent(RuntimeOrigin::signed(renter1), 0));
        {
            // - confirm_rent()
            // - Writes: RentInfo, ConfirmingOrder, UserTotakStake, Balance,
            assert_eq!(
                RentMachine::rent_info(0),
                Some(RentOrderDetail {
                    machine_id: machine_id.clone(),
                    renter: renter1,
                    rent_start: 11,
                    confirm_rent: 11,
                    rent_end: 11 + ONE_DAY,
                    stake_amount: 0,
                    rent_status: RentStatus::Renting,
                    gpu_num: 2,
                    gpu_index: vec![0, 1],
                })
            );
            // assert_eq!(RentMachine::confirming_order(&0), );
            assert!(!<ConfirmingOrder::<TestRuntime>>::contains_key(&0));
            assert_eq!(RentMachine::user_total_stake(renter1), 0);

            // online_profile:
            // MachinesInfo(total_rent_times, machine_status); LiveMachines, PosGPUInfo,
            // EraStashPoints, ErasMachinePoints, SysInfo, StashMachines

            assert_eq!(
                OnlineProfile::eras_stash_points(1),
                EraStashPoints { ..Default::default() }
            );
            assert_eq!(
                OnlineProfile::live_machines(),
                LiveMachine { rented_machine: vec![machine_id.clone()], ..Default::default() }
            );
            // assert_eq!(OnlineProfile::eras_machine_points(1), );
            // FIXME: should add total_stake if someone rent machine
            assert_eq!(
                OnlineProfile::sys_info(),
                SysInfoDetail {
                    total_gpu_num: 4,
                    total_rented_gpu: 4,
                    total_staker: 1,
                    total_calc_points: 77881,
                    total_stake: 40000 * ONE_DBC,
                    // [Thread B ③] 托管：确认时不计租金收益（钱在托管、可退），结算才入账 → 此刻为 0
                    total_rent_fee: 0,
                    total_burn_fee: 0,
                    ..Default::default()
                }
            );
            assert_eq!(
                OnlineProfile::stash_machines(machine_info.machine_stash),
                StashMachine {
                    total_machine: vec![machine_id.clone()],
                    online_machine: vec![machine_id.clone()],
                    total_calc_points: 77881,
                    total_gpu_num: 4,
                    total_rented_gpu: 4,
                    // [Thread B ③] 托管：确认时不计租金收益（钱在托管、可退），结算才入账 → 此刻为 0
                    total_rent_fee: 0,
                    total_burn_fee: 0,
                    ..Default::default()
                }
            );
        }

        // NOTE: 确保机器得分不会改变两次！
        assert_ok!(RentMachine::confirm_rent(RuntimeOrigin::signed(renter2), 1));
        {
            // - confirm_rent()
            // - Writes: RentInfo, ConfirmingOrder, UserTotakStake, Balance,
            assert_eq!(
                RentMachine::rent_info(1),
                Some(RentOrderDetail {
                    machine_id: machine_id.clone(),
                    renter: renter2,
                    rent_start: 11,
                    confirm_rent: 11,
                    rent_end: 11 + ONE_DAY,
                    stake_amount: 0,
                    rent_status: RentStatus::Renting,
                    gpu_num: 2,
                    gpu_index: vec![2, 3],
                })
            );
            // assert_eq!(RentMachine::confirming_order(&0), );
            assert!(!<ConfirmingOrder::<TestRuntime>>::contains_key(&1));
            assert_eq!(RentMachine::user_total_stake(renter2), 0);

            // online_profile:
            // MachinesInfo(total_rent_times, machine_status); LiveMachines, PosGPUInfo,
            // EraStashPoints, ErasMachinePoints, SysInfo, StashMachines

            let machine_info = OnlineProfile::machines_info(&machine_id).unwrap();
            assert_eq!(machine_info.renters, vec![renter2, renter1]);
            assert_eq!(
                OnlineProfile::eras_stash_points(1),
                EraStashPoints { ..Default::default() }
            );
            assert_eq!(
                OnlineProfile::live_machines(),
                LiveMachine { rented_machine: vec![machine_id.clone()], ..Default::default() }
            );
            // assert_eq!(OnlineProfile::eras_machine_points(1), );
            assert_eq!(
                OnlineProfile::sys_info(),
                SysInfoDetail {
                    total_gpu_num: 4,
                    total_rented_gpu: 4,
                    total_staker: 1,
                    total_calc_points: 77881,
                    total_stake: 40000 * ONE_DBC,
                    // [Thread B ③] 托管：两单确认后仍未结算 → 计数为 0
                    total_rent_fee: 0,
                    total_burn_fee: 0,
                    ..Default::default()
                }
            );
            assert_eq!(
                OnlineProfile::stash_machines(machine_info.machine_stash),
                StashMachine {
                    total_machine: vec![machine_id.clone()],
                    online_machine: vec![machine_id.clone()],
                    total_calc_points: 77881,
                    total_gpu_num: 4,
                    // NOTE: 这里应该记录为4
                    total_rented_gpu: 4,
                    // [Thread B ③] 托管：两单确认后仍未结算 → 计数为 0
                    total_rent_fee: 0,
                    total_burn_fee: 0,
                    ..Default::default()
                }
            );
        }

        // 租用人1续租1天
        assert_ok!(RentMachine::relet_machine(RuntimeOrigin::signed(renter1), 0, 1 * ONE_DAY));
        {
            // relet_machine:
            // - Writes: OrderInfo, Balance, RentEnding,
            //
            // OnlineProfile:
            // SysInfo, StashMachines, MachinesInfo,
            assert_eq!(
                RentMachine::rent_info(0),
                Some(RentOrderDetail {
                    machine_id: machine_id.clone(),
                    renter: renter1,
                    rent_start: 11,
                    confirm_rent: 11,
                    rent_end: 11 + 2 * ONE_DAY,
                    stake_amount: 0,
                    rent_status: RentStatus::Renting,
                    gpu_num: 2,
                    gpu_index: vec![0, 1],
                })
            );

            assert_eq!(RentMachine::rent_ending(11 + ONE_DAY), vec![1]);
            assert_eq!(RentMachine::rent_ending(11 + 2 * ONE_DAY), vec![0]);

            assert_eq!(
                OnlineProfile::sys_info(),
                SysInfoDetail {
                    total_gpu_num: 4,
                    total_rented_gpu: 4,
                    total_staker: 1,
                    total_calc_points: 77881,
                    total_stake: 40000 * ONE_DBC,
                    // [Thread B ③] 托管：续租也进托管，结算才入账 → 此刻仍为 0
                    total_rent_fee: 0,
                    total_burn_fee: 0,
                    ..Default::default()
                }
            );
            let machine_info = OnlineProfile::machines_info(&machine_id).unwrap();
            assert_eq!(
                OnlineProfile::stash_machines(machine_info.machine_stash),
                StashMachine {
                    total_machine: vec![machine_id.clone()],
                    online_machine: vec![machine_id.clone()],
                    total_calc_points: 77881,
                    total_gpu_num: 4,
                    // NOTE: 这里应该记录为4
                    total_rented_gpu: 4,
                    // [Thread B ③] 托管：续租也进托管，结算才入账 → 此刻仍为 0
                    total_rent_fee: 0,
                    total_burn_fee: 0,
                    ..Default::default()
                }
            );
        }

        let live_machines = OnlineProfile::live_machines();
        assert!(live_machines.rented_machine.binary_search(&machine_id).is_ok());

        // 过一天，租用人2到期
        run_to_block(12 + ONE_DAY);
        {
            // TODO: 确保机器得分不改变
            // change_machine_status_on_rent_end
            // MachinesInfo, LiveMachines, MachineRentedGPU
            assert_eq!(OnlineProfile::machine_rented_gpu(&machine_id), 2);
            let live_machines = OnlineProfile::live_machines();
            assert!(live_machines.online_machine.binary_search(&machine_id).is_err());
            assert!(live_machines.rented_machine.binary_search(&machine_id).is_ok());
            let machine_info = OnlineProfile::machines_info(&machine_id).unwrap();
            assert_eq!(machine_info.machine_status, MachineStatus::Rented);
            assert_eq!(machine_info.total_rented_duration, 12 * ONE_HOUR);
            assert_eq!(machine_info.renters, vec![renter1]);

            // clean_order
            // -Write: MachineRentOrder, RentEnding, RentInfo,
            // UserOrder, ConfirmingOrder
            assert_eq!(
                RentMachine::machine_rent_order(&machine_id),
                MachineGPUOrder { rent_order: vec![0], used_gpu: vec![0, 1] }
            );

            let user_order: Vec<RentOrderId> = vec![];
            assert_eq!(RentMachine::rent_ending(12 + ONE_DAY), user_order.clone());
            assert!(!<RentInfo::<TestRuntime>>::contains_key(&1));
            assert_eq!(RentMachine::user_order(renter2), user_order);
            assert!(!<ConfirmingOrder::<TestRuntime>>::contains_key(&1));
        }

        // TODO: 租用人1进行举报

        // 再过了一天，租用人1到期
        run_to_block(12 + 2 * ONE_DAY);
        {
            // TODO: 确保得分，等一些信息被还原
            // change_machine_status_on_rent_end
            // MachinesInfo, LiveMachines, MachineRentedGPU
            assert_eq!(OnlineProfile::machine_rented_gpu(&machine_id), 0);
            let live_machines = OnlineProfile::live_machines();
            assert!(live_machines.online_machine.binary_search(&machine_id).is_ok());
            assert!(live_machines.rented_machine.binary_search(&machine_id).is_err());
            let machine_info = OnlineProfile::machines_info(&machine_id).unwrap();
            assert_eq!(machine_info.machine_status, MachineStatus::Online);
            assert_eq!(machine_info.total_rented_duration, 36 * ONE_HOUR);
            assert_eq!(machine_info.renters, vec![]);

            // clean_order
            // -Write: MachineRentOrder, RentEnding, RentInfo,
            // UserOrder, ConfirmingOrder
            assert_eq!(RentMachine::machine_rent_order(&machine_id), MachineGPUOrder::default());

            let user_order: Vec<RentOrderId> = vec![];
            assert_eq!(RentMachine::rent_ending(12 + ONE_DAY), user_order.clone());
            assert!(!<RentInfo::<TestRuntime>>::contains_key(&1));
            assert_eq!(RentMachine::user_order(renter2), user_order);
            assert!(!<ConfirmingOrder::<TestRuntime>>::contains_key(&1));

            // [Thread B ③] 两单都到期、托管全额结算(全程 100% 已用、无罚)后，生命周期计数应
            //   与旧「确认即付」模型的累计值完全一致 —— 证明托管只是把入账时点从 confirm 移到 settle，
            //   不改变最终经济结果（守恒）。数值 = 旧测试续租后的累计(此后无新账单)。
            assert_eq!(OnlineProfile::sys_info().total_rent_fee, 35559687499999999998);
            assert_eq!(OnlineProfile::sys_info().total_burn_fee, 1871562500000000001);
        }
    })
}
