#![allow(dead_code)]

use super::*;
use crate::{mock::*, OCCommitteeMachineList, OCMachineCommitteeList};
use committee::CommitteeList;
use frame_support::assert_ok;
use online_profile::{
    CommitteeUploadInfo, EraStashPoints, LiveMachine, MachineGradeStatus, MachineStatus, StakerCustomizeInfo,
    UserMutHardwareStakeInfo,
};
use sp_runtime::Perbill;
use std::{collections::BTreeMap, convert::TryInto};

#[test]
fn machine_online_works() {
    new_test_with_init_params_ext().execute_with(|| {
        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One).into();
        let committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Two).into();
        let committee3: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Dave).into();

        let controller: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();
        let stash: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();
        // Bob pubkey
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();
        let msg = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48\
                   5CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL";
        let sig = "b4084f70730b183127e9db78c6d8dcf79039f23466cd1ee8b536c40c3027a83d\
                   ab040be4ed2db57b67eaac406817a69ce72a13f8ac11ba460e15d318b1504481";

        // 查询状态
        assert_eq!(Balances::free_balance(committee1), INIT_BALANCE);
        assert_eq!(DBCPriceOCW::avg_price(), Some(12_000u64));

        // stash 账户设置控制账户
        assert_ok!(OnlineProfile::set_controller(Origin::signed(stash), controller));

        // controller 生成server_name
        assert_ok!(OnlineProfile::gen_server_room(Origin::signed(controller)));
        assert_ok!(OnlineProfile::gen_server_room(Origin::signed(controller)));

        let server_room = OnlineProfile::stash_server_rooms(&stash);

        assert_ok!(OnlineProfile::bond_machine(
            Origin::signed(controller),
            machine_id.clone(),
            msg.as_bytes().to_vec(),
            hex::decode(sig).unwrap()
        ));

        let mut machine_info = online_profile::MachineInfo {
            controller: controller.clone(),
            machine_stash: stash.clone(),
            bonding_height: 3,
            stake_amount: 100000 * ONE_DBC,
            init_stake_per_gpu: 100000 * ONE_DBC,
            machine_status: online_profile::MachineStatus::AddingCustomizeInfo,
            ..Default::default()
        };

        // bond_machine:
        // - Writes: ControllerMachines, StashMachines, LiveMachines, MachinesInfo, SysInfo, StashStake

        let stash_machine_info =
            online_profile::StashMachine { total_machine: vec![machine_id.clone()], ..Default::default() };
        assert_eq!(OnlineProfile::controller_machines(&controller), vec!(machine_id.clone()));
        assert_eq!(&OnlineProfile::stash_machines(&stash), &stash_machine_info);
        assert_eq!(
            OnlineProfile::live_machines(),
            LiveMachine { bonding_machine: vec!(machine_id.clone()), ..Default::default() }
        );
        assert_eq!(OnlineProfile::machines_info(&machine_id), machine_info.clone());
        assert_eq!(
            OnlineProfile::sys_info(),
            online_profile::SysInfoDetail { total_staker: 0, total_stake: 100000 * ONE_DBC, ..Default::default() }
        );
        assert_eq!(OnlineProfile::stash_stake(&stash), 100000 * ONE_DBC);
        // 查询Controller支付30 DBC手续费: 绑定机器/添加机房信息各花费10DBC
        assert_eq!(Balances::free_balance(controller), INIT_BALANCE - 30 * ONE_DBC);

        let customize_info = StakerCustomizeInfo {
            server_room: server_room[0].clone(),
            upload_net: 100,
            download_net: 100,
            longitude: online_profile::Longitude::East(1157894),
            latitude: online_profile::Latitude::North(235678),
            telecom_operators: vec!["China Unicom".into()],
        };
        // 控制账户添加机器信息
        assert_ok!(OnlineProfile::add_machine_info(
            Origin::signed(controller),
            machine_id.clone(),
            customize_info.clone()
        ));

        machine_info.machine_info_detail.staker_customize_info = customize_info.clone();
        machine_info.machine_status = online_profile::MachineStatus::DistributingOrder;

        run_to_block(3);

        // 添加了信息之后，将会在OnlineCommittee中被派单
        // add_machine_info
        // - Writes: MachinesInfo, LiveMachines, committee::CommitteeStake
        assert_eq!(&OnlineProfile::machines_info(&machine_id), &machine_info);
        assert_eq!(
            OnlineProfile::live_machines(),
            LiveMachine { confirmed_machine: vec!(machine_id.clone()), ..Default::default() }
        );

        // 增加一个委员会
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee1));
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee2));
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee3));

        let one_box_pubkey: [u8; 32] = hex::decode("9dccbab2d61405084eac440f877a6479bc827373b2e414e81a6170ebe5aadd12")
            .unwrap()
            .try_into()
            .unwrap();
        assert_ok!(Committee::committee_set_box_pubkey(Origin::signed(committee1), one_box_pubkey.clone()));
        assert_ok!(Committee::committee_set_box_pubkey(Origin::signed(committee2), one_box_pubkey.clone()));
        assert_ok!(Committee::committee_set_box_pubkey(Origin::signed(committee3), one_box_pubkey.clone()));

        // 委员会处于正常状态(排序后的列表)
        assert_eq!(
            Committee::committee(),
            CommitteeList { normal: vec![committee2, committee3, committee1], ..Default::default() }
        );
        // 获取可派单的委员会正常
        assert_ok!(OnlineCommittee::committee_workflow().ok_or(()));

        run_to_block(5);

        let mut committee_stake_info = committee::CommitteeStakeInfo {
            box_pubkey: one_box_pubkey,
            staked_amount: 20000 * ONE_DBC,
            used_stake: 1000 * ONE_DBC,
            ..Default::default()
        };

        machine_info.machine_status = online_profile::MachineStatus::CommitteeVerifying;

        // Do distribute_machines:
        // - Writes: op::MachinesInfo, op::LiveMachines, committee::CommitteeStake,
        // lc::MachineCommittee, lc::CommitteeMachine, lc::CommitteeOps
        assert_eq!(&Committee::committee_stake(&committee1), &committee_stake_info);
        assert_eq!(
            OnlineProfile::live_machines(),
            online_profile::LiveMachine { booked_machine: vec![machine_id.clone()], ..Default::default() }
        );
        assert_eq!(OnlineProfile::machines_info(&machine_id), machine_info);

        assert_eq!(
            OnlineCommittee::machine_committee(machine_id.clone()),
            OCMachineCommitteeList {
                book_time: 4,
                confirm_start_time: 4324,
                booked_committee: vec![committee2, committee3, committee1],
                ..Default::default()
            }
        );
        assert_eq!(
            OnlineCommittee::committee_machine(&committee1),
            crate::OCCommitteeMachineList { booked_machine: vec![machine_id.clone()], ..Default::default() }
        );
        assert_eq!(
            OnlineCommittee::committee_ops(&committee1, &machine_id),
            crate::OCCommitteeOps {
                staked_dbc: 1000 * ONE_DBC,
                verify_time: vec![4, 1444, 2884],
                ..Default::default()
            }
        );

        // 委员会提交机器Hash
        let machine_info_hash1: [u8; 16] = hex::decode("fd8885a22a9d9784adaa36effcd77522").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(committee1),
            machine_id.clone(),
            machine_info_hash1
        ));
        let machine_info_hash2: [u8; 16] = hex::decode("c016090e0943c17f5d4999dc6eb52683").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(committee2),
            machine_id.clone(),
            machine_info_hash2
        ));
        let machine_info_hash3: [u8; 16] = hex::decode("4a6b2df1e1a77b9bcdab5e31dc7950d2").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(committee3),
            machine_id.clone(),
            machine_info_hash3
        ));

        let mut committee_upload_info = CommitteeUploadInfo {
            machine_id: machine_id.clone(),
            gpu_type: "GeForceRTX3080".as_bytes().to_vec(),
            gpu_num: 4,
            cuda_core: 8704,
            gpu_mem: 10,
            calc_point: 59890,
            sys_disk: 500,
            data_disk: 3905,
            cpu_type: "Intel(R) Xeon(R) Silver 4214R".as_bytes().to_vec(),
            cpu_core_num: 46,
            cpu_rate: 2400,
            mem_num: 440,

            rand_str: "abcdefg1".as_bytes().to_vec(),
            is_support: true,
        };

        // 委员会提交原始信息
        assert_ok!(OnlineCommittee::submit_confirm_raw(Origin::signed(committee1), committee_upload_info.clone()));
        committee_upload_info.rand_str = "abcdefg2".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(Origin::signed(committee2), committee_upload_info.clone()));
        committee_upload_info.rand_str = "abcdefg3".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(Origin::signed(committee3), committee_upload_info.clone()));

        // submit_confirm_raw:
        // - Writes: MachineSubmitedHash, MachineCommittee, CommitteeMachine, CommitteeOps
        assert_eq!(
            OnlineCommittee::machine_submited_hash(&machine_id),
            vec![machine_info_hash3, machine_info_hash2, machine_info_hash1]
        );
        assert_eq!(
            OnlineCommittee::machine_committee(&machine_id),
            crate::OCMachineCommitteeList {
                book_time: 4,
                confirm_start_time: 4324,
                booked_committee: vec![committee2, committee3, committee1],
                hashed_committee: vec![committee2, committee3, committee1],
                confirmed_committee: vec![committee2, committee3, committee1],
                status: crate::OCVerifyStatus::Summarizing,
                ..Default::default()
            }
        );
        assert_eq!(
            OnlineCommittee::committee_machine(&committee1),
            crate::OCCommitteeMachineList { confirmed_machine: vec![machine_id.clone()], ..Default::default() }
        );
        committee_upload_info.rand_str = vec![];
        assert_eq!(
            OnlineCommittee::committee_ops(&committee1, machine_id.clone()),
            crate::OCCommitteeOps {
                staked_dbc: 1000 * ONE_DBC,
                verify_time: vec![4, 1444, 2884],
                confirm_hash: machine_info_hash1,
                hash_time: 6,
                confirm_time: 6,
                machine_status: crate::OCMachineStatus::Confirmed,
                machine_info: committee_upload_info.clone(),
            }
        );

        run_to_block(10);

        // Statistic_result & Online:
        // - Writes: CommitteeMachine, StashMachineStaske
        // LiveMachines, MachinesInfo, PosGPU, ServerRoomGPU, SysInfo, StashMachines, ErasStashPoints, ErasMachinePoints,

        // 检查机器状态
        assert_eq!(
            OnlineProfile::live_machines(),
            LiveMachine { online_machine: vec!(machine_id.clone()), ..Default::default() }
        );

        let mut machine_info = online_profile::MachineInfo {
            machine_status: online_profile::MachineStatus::Online,
            last_machine_restake: 6,
            online_height: 6,
            last_online_height: 6,
            stake_amount: 400000 * ONE_DBC,
            reward_deadline: 365 * 2,
            reward_committee: vec![committee2, committee3, committee1],
            machine_info_detail: online_profile::MachineInfoDetail {
                committee_upload_info,
                staker_customize_info: machine_info.machine_info_detail.staker_customize_info,
            },
            ..machine_info
        };

        let sys_info = online_profile::SysInfoDetail {
            total_gpu_num: 4,
            total_staker: 1,
            total_calc_points: 59914, // 59890 + 59890 * 4/10000) = +24
            total_stake: 400000 * ONE_DBC,
            ..Default::default()
        };

        assert_eq!(&OnlineProfile::machines_info(&machine_id), &machine_info);
        assert_eq!(
            OnlineProfile::pos_gpu_info(
                online_profile::Longitude::East(1157894),
                online_profile::Latitude::North(235678)
            ),
            online_profile::PosInfo { online_gpu: 4, online_gpu_calc_points: 59890, ..Default::default() }
        );
        assert_eq!(&OnlineProfile::sys_info(), &sys_info);

        let mut staker_statistic = BTreeMap::new();
        staker_statistic.insert(
            stash,
            online_profile::StashMachineStatistics {
                online_gpu_num: 4,
                inflation: Perbill::from_rational_approximation(4u32, 10000),
                machine_total_calc_point: 59890,
                rent_extra_grade: 0,
            },
        );
        assert_eq!(OnlineProfile::eras_stash_points(0), EraStashPoints { ..Default::default() });
        assert_eq!(
            OnlineProfile::eras_stash_points(1),
            EraStashPoints { total: 59914, staker_statistic: staker_statistic.clone() }
        );

        let mut era_machine_points = BTreeMap::new();
        assert_eq!(OnlineProfile::eras_machine_points(0), BTreeMap::new());
        era_machine_points.insert(machine_id.clone(), MachineGradeStatus { basic_grade: 59890, is_rented: false });
        assert_eq!(OnlineProfile::eras_machine_points(1), era_machine_points);

        // 过一个Era: 一天是2880个块
        run_to_block(2880 * 2 + 2);

        // do distribute_reward
        // - Writes:
        // ErasMachineReleasedReward, ErasMachineReward
        // ErasStashReleasedReward, ErasStashReward, StashMachines, committee reward

        assert_eq!(OnlineProfile::eras_machine_reward(0, &machine_id), 0);
        assert_eq!(OnlineProfile::eras_machine_reward(1, &machine_id), 1089000 * ONE_DBC); // 1100000 * 0.99
        assert_eq!(OnlineProfile::eras_machine_released_reward(0, &machine_id), 0);
        assert_eq!(OnlineProfile::eras_machine_released_reward(1, &machine_id), 272250 * ONE_DBC); // 1100000 * 0.99 * 0.25

        assert_eq!(OnlineProfile::eras_stash_reward(0, &stash), 0);
        assert_eq!(OnlineProfile::eras_stash_reward(1, &stash), 1089000 * ONE_DBC);
        assert_eq!(OnlineProfile::eras_stash_released_reward(0, &stash), 0);
        assert_eq!(OnlineProfile::eras_stash_released_reward(1, &stash), 272250 * ONE_DBC); // 1100000 * 0.99 * 0.25

        let stash_machine_info = online_profile::StashMachine {
            can_claim_reward: 272250 * ONE_DBC, // 1100000 * 0.99 * 0.25
            online_machine: vec![machine_id.clone()],
            total_earned_reward: 1089000 * ONE_DBC,
            total_calc_points: 59914,
            total_gpu_num: 4,
            ..stash_machine_info
        };

        assert_eq!(&OnlineProfile::stash_machines(stash), &stash_machine_info);

        // 1100000 * 25 / 100 / 100 * (333333333 / 10**9) = 916.66666575
        committee_stake_info.can_claim_reward = 916 * ONE_DBC + 66666575 * 10_000_000;
        committee_stake_info.used_stake = 0;
        assert_eq!(&Committee::committee_stake(&committee1), &committee_stake_info);

        // 再次释放
        run_to_block(2880 * 3 + 2);

        // 线性释放
        // do distribute_reward
        // - Writes:
        // ErasMachineReleasedReward, ErasMachineReward
        // ErasStashReleasedReward, ErasStashReward, StashMachines, committee reward

        assert_eq!(OnlineProfile::eras_machine_reward(0, &machine_id), 0);
        assert_eq!(OnlineProfile::eras_machine_reward(1, &machine_id), 1089000 * ONE_DBC); // 1100000 * 0.99
        assert_eq!(OnlineProfile::eras_machine_reward(2, &machine_id), 1089000 * ONE_DBC); // 1100000 * 0.99
        assert_eq!(OnlineProfile::eras_machine_released_reward(0, &machine_id), 0);
        assert_eq!(OnlineProfile::eras_machine_released_reward(1, &machine_id), 272250 * ONE_DBC); // 1100000 * 0.99 * 0.25

        // 释放剩余奖励的1/150: 1100000 * 0.75 / 150 * 0.99 = 5445;
        // NOTE: 实际上，= 1100000 * 0.75 * 6666666 / 10**9 * 0.99 =  5444.9994555 DBC
        // 第一天奖励，在第二天线性释放，委员会获得的部分:
        let first_day_linear_release = 5444 * ONE_DBC + 9994555 * ONE_DBC / 10000000;
        assert_eq!(
            OnlineProfile::eras_machine_released_reward(2, &machine_id),
            272250 * ONE_DBC + first_day_linear_release
        ); // 1100000 * 0.99 * 0.25 + 1100000 * 0.75 * 0.99 / 150

        let mut stash_machine_info = online_profile::StashMachine {
            can_claim_reward: 272250 * 2 * ONE_DBC + first_day_linear_release, // 1100000 * 0.99 * 0.25
            online_machine: vec![machine_id.clone()],
            total_earned_reward: 1089000 * ONE_DBC * 2,
            total_calc_points: 59914,
            total_gpu_num: 4,
            ..stash_machine_info
        };

        assert_eq!(OnlineProfile::eras_stash_reward(0, &stash), 0);
        assert_eq!(OnlineProfile::eras_stash_reward(1, &stash), 1089000 * ONE_DBC);
        assert_eq!(OnlineProfile::eras_stash_reward(2, &stash), 1089000 * ONE_DBC);
        assert_eq!(OnlineProfile::eras_stash_released_reward(0, &stash), 0);
        assert_eq!(OnlineProfile::eras_stash_released_reward(1, &stash), 272250 * ONE_DBC); // 1100000 * 0.99 * 0.25
        assert_eq!(OnlineProfile::eras_stash_released_reward(2, &stash), 272250 * ONE_DBC + first_day_linear_release); // 1100000 * 0.99 * 0.25

        assert_eq!(&OnlineProfile::stash_machines(stash), &stash_machine_info);

        // 第二天释放的获得的第一天的将奖励： 1100000 * 0.75 * 6666666 / 10**9 * 0.01 = 54.9999945
        // 916 * ONE_DBC + 66666575 * 10_000_000 = 916.66666575; (1/150 -> 6666666 / 10**9), 1100000 * 0.75 * (6666666 / 10**9) * 0.01 * 333333333 / 10**9 = 18.33333148166667
        committee_stake_info.can_claim_reward =
            (916 * ONE_DBC + 66666575 * 10_000_000) * 2 + 18 * ONE_DBC + 333331481666668;
        committee_stake_info.used_stake = 0;
        assert_eq!(&Committee::committee_stake(&committee1), &committee_stake_info);

        stash_machine_info.total_earned_reward = 2178000 * ONE_DBC; // 1100000 * 0.99 * 2
        assert_eq!(&OnlineProfile::stash_machines(&stash), &stash_machine_info);

        // 矿工领取奖励
        // - Writes:
        // StashMachines, User Balances
        assert_ok!(OnlineProfile::claim_rewards(Origin::signed(controller)));
        // 领取奖励后，查询剩余奖励
        stash_machine_info.total_claimed_reward = stash_machine_info.can_claim_reward;
        stash_machine_info.can_claim_reward = 0;
        assert_eq!(&OnlineProfile::stash_machines(&stash), &stash_machine_info);

        // 领取奖励后，查询账户余额
        assert_eq!(Balances::free_balance(stash), INIT_BALANCE - 400000 * ONE_DBC + 549944999455500000000);

        // 委员会领取奖励
        // - Writes:
        // CommitteeStake, Committee Balance
        assert_ok!(Committee::claim_reward(Origin::signed(committee1)));
        committee_stake_info.claimed_reward = committee_stake_info.can_claim_reward;
        committee_stake_info.can_claim_reward = 0;
        assert_eq!(&Committee::committee_stake(&committee1), &committee_stake_info);
        let current_committee1_balance = INIT_BALANCE - 20000 * ONE_DBC + committee_stake_info.claimed_reward;
        assert_eq!(Balances::free_balance(committee1), current_committee1_balance);

        // NOTE: 测试 控制账户重新上线机器
        assert_ok!(OnlineProfile::offline_machine_change_hardware_info(Origin::signed(controller), machine_id.clone()));

        // - Writes:
        // LiveMachines, MachineInfo, StashStake, UserMutHardwarestake, PosGPUInfo, StashMachine
        // CurrentEraStashPoints, NextEraStashPoints, CurrentEraMachinePoints, NextEraMachinePoints, SysInfo,
        assert_eq!(
            OnlineProfile::live_machines(),
            LiveMachine { bonding_machine: vec![machine_id.clone()], ..Default::default() }
        );
        machine_info.machine_status = MachineStatus::StakerReportOffline(8643, Box::new(MachineStatus::Online));
        assert_eq!(&OnlineProfile::machines_info(&machine_id), &machine_info);
        assert_eq!(OnlineProfile::stash_stake(&stash), 2000 * ONE_DBC + 400000 * ONE_DBC);
        assert_eq!(
            OnlineProfile::user_mut_hardware_stake(&stash, &machine_id),
            online_profile::UserMutHardwareStakeInfo { stake_amount: 2000 * ONE_DBC, offline_time: 2880 * 3 + 3 }
        );
        assert_eq!(
            OnlineProfile::pos_gpu_info(
                online_profile::Longitude::East(1157894),
                online_profile::Latitude::North(235678)
            ),
            online_profile::PosInfo { offline_gpu: 4, ..Default::default() }
        );

        assert_eq!(
            OnlineProfile::sys_info(),
            online_profile::SysInfoDetail { total_stake: (400000 + 2000) * ONE_DBC, ..Default::default() }
        );
        stash_machine_info.online_machine = vec![];
        stash_machine_info.total_gpu_num = 0;
        stash_machine_info.total_calc_points = 0;
        assert_eq!(&OnlineProfile::stash_machines(&stash), &stash_machine_info);

        // 当前Era为3
        assert_eq!(OnlineProfile::current_era(), 3);
        assert_eq!(OnlineProfile::eras_stash_points(3), EraStashPoints { ..Default::default() });
        assert_eq!(OnlineProfile::eras_stash_points(4), EraStashPoints { ..Default::default() });
        assert_eq!(OnlineProfile::eras_machine_points(3), BTreeMap::new());
        assert_eq!(OnlineProfile::eras_machine_points(4), BTreeMap::new());

        // 控制账户重新添加机器信息
        assert_ok!(OnlineProfile::add_machine_info(
            Origin::signed(controller),
            machine_id.clone(),
            StakerCustomizeInfo {
                server_room: server_room[0],
                upload_net: 100,
                download_net: 100,
                longitude: online_profile::Longitude::East(1157894),
                latitude: online_profile::Latitude::North(235678),
                telecom_operators: vec!["China Unicom".into()],
            }
        ));

        run_to_block(2880 * 3 + 3);

        // 委员会审核机器重新上链

        // Do distribute_machines:
        // - Writes: op::MachinesInfo, op::LiveMachines, committee::CommitteeStake,
        // lc::MachineCommittee, lc::CommitteeMachine, lc::CommitteeOps

        // 查询机器中有订阅的委员会
        let committee_stake_info = committee::CommitteeStakeInfo {
            box_pubkey: one_box_pubkey,
            staked_amount: 20000 * ONE_DBC,
            used_stake: 1000 * ONE_DBC,
            ..committee_stake_info
        };
        assert_eq!(&Committee::committee_stake(&committee1), &committee_stake_info);
        assert_eq!(
            OnlineProfile::live_machines(),
            online_profile::LiveMachine { booked_machine: vec![machine_id.clone()], ..Default::default() }
        );
        machine_info.machine_status = MachineStatus::CommitteeVerifying;
        assert_eq!(OnlineProfile::machines_info(&machine_id), machine_info);

        assert_eq!(
            OnlineCommittee::machine_committee(machine_id.clone()),
            OCMachineCommitteeList {
                book_time: 2880 * 3 + 3,
                confirm_start_time: 2880 * 3 + 3 + 4320,
                booked_committee: vec![committee2, committee3, committee1],
                ..Default::default()
            }
        );
        assert_eq!(
            OnlineCommittee::committee_machine(&committee1),
            crate::OCCommitteeMachineList {
                booked_machine: vec![machine_id.clone()],
                online_machine: vec![machine_id.clone()],
                ..Default::default()
            }
        );
        assert_eq!(
            OnlineCommittee::committee_ops(&committee1, &machine_id),
            crate::OCCommitteeOps {
                staked_dbc: 1000 * ONE_DBC,
                verify_time: vec![8643, 10083, 11523],
                ..Default::default()
            }
        );

        // 委员会提交机器Hash
        let machine_info_hash1: [u8; 16] = hex::decode("53cf058dfa07ef517b2f28bccff88c2b").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(committee1),
            machine_id.clone(),
            machine_info_hash1.clone()
        ));

        let machine_info_hash2: [u8; 16] = hex::decode("3f775d3f4a144b94d6d551f6091a5126").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(committee2),
            machine_id.clone(),
            machine_info_hash2.clone()
        ));

        let machine_info_hash3: [u8; 16] = hex::decode("4983040157403addac94ca860ddbff7f").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(committee3),
            machine_id.clone(),
            machine_info_hash3.clone()
        ));

        // submit_confirm_hash:
        // - Writes:
        // MachineSubmitedHash, MachineCommittee, CommitteeMachine, CommitteeOps

        // 委员会提交原始信息
        let mut committee_upload_info = CommitteeUploadInfo {
            machine_id: machine_id.clone(),
            gpu_type: "GeForceRTX3080".as_bytes().to_vec(),
            gpu_num: 8,
            cuda_core: 8704,
            gpu_mem: 10,
            calc_point: 119780,
            sys_disk: 500,
            data_disk: 3905,
            cpu_type: "Intel(R) Xeon(R) Silver 4214R".as_bytes().to_vec(),
            cpu_core_num: 46,
            cpu_rate: 2400,
            mem_num: 440,

            rand_str: "abcdefg1".as_bytes().to_vec(),
            is_support: true,
        };
        assert_ok!(&OnlineCommittee::submit_confirm_raw(Origin::signed(committee1), committee_upload_info.clone()));
        committee_upload_info.rand_str = "abcdefg2".as_bytes().to_vec();
        assert_ok!(&OnlineCommittee::submit_confirm_raw(Origin::signed(committee2), committee_upload_info.clone()));
        committee_upload_info.rand_str = "abcdefg3".as_bytes().to_vec();
        assert_ok!(&OnlineCommittee::submit_confirm_raw(Origin::signed(committee3), committee_upload_info.clone()));

        // submit_confirm_raw:
        // - Writes: MachineSubmitedHash, MachineCommittee, CommitteeMachine, CommitteeOps
        assert_eq!(
            OnlineCommittee::machine_submited_hash(&machine_id),
            vec![machine_info_hash2, machine_info_hash3, machine_info_hash1]
        );
        assert_eq!(
            OnlineCommittee::machine_committee(&machine_id),
            crate::OCMachineCommitteeList {
                book_time: 8643,
                confirm_start_time: 12963,
                booked_committee: vec![committee2, committee3, committee1],
                hashed_committee: vec![committee2, committee3, committee1],
                confirmed_committee: vec![committee2, committee3, committee1],
                status: crate::OCVerifyStatus::Summarizing,
                ..Default::default()
            }
        );
        assert_eq!(
            OnlineCommittee::committee_machine(&committee1),
            crate::OCCommitteeMachineList {
                confirmed_machine: vec![machine_id.clone()],
                online_machine: vec![machine_id.clone()],
                ..Default::default()
            }
        );
        committee_upload_info.rand_str = vec![];
        assert_eq!(
            OnlineCommittee::committee_ops(&committee1, machine_id.clone()),
            crate::OCCommitteeOps {
                staked_dbc: 1000 * ONE_DBC,
                verify_time: vec![8643, 10083, 11523],
                confirm_hash: machine_info_hash1,
                hash_time: 8644,
                confirm_time: 8644,
                machine_status: crate::OCMachineStatus::Confirmed,
                machine_info: committee_upload_info.clone(),
            }
        );

        run_to_block(2880 * 3 + 4);

        // Will do lc_confirm_machine
        // - Writes:
        // MachinesInfo, LiveMachines, SysInfo, StashStake, ServerRoomMachines, PosGPUInfo, EraMachineSnap, EraStashSnap
        // CommitteeReward(get reward immediately), UserReonlineStake, do Slash

        committee_upload_info.rand_str = vec![];
        machine_info.machine_info_detail.committee_upload_info = committee_upload_info;
        let machine_info = online_profile::MachineInfo {
            last_machine_restake: 8644,
            last_online_height: 8644,
            stake_amount: 800000 * ONE_DBC,
            machine_status: online_profile::MachineStatus::Online,
            ..machine_info
        };

        assert_eq!(&OnlineProfile::machines_info(&machine_id), &machine_info);
        assert_eq!(
            OnlineProfile::live_machines(),
            LiveMachine { online_machine: vec!(machine_id.clone()), ..Default::default() }
        );
        assert_eq!(
            OnlineProfile::sys_info(),
            online_profile::SysInfoDetail {
                total_gpu_num: 8,
                total_staker: 1,
                total_calc_points: 119876, // 119780 + 119780 * 8 /10000 = 119875.824
                total_stake: 800000 * ONE_DBC,
                ..Default::default()
            }
        );
        assert_eq!(
            OnlineProfile::user_mut_hardware_stake(&stash, &machine_id),
            UserMutHardwareStakeInfo { ..Default::default() }
        );
        assert_eq!(OnlineProfile::stash_stake(&stash), 800000 * ONE_DBC);
        // 检查分数

        let mut staker_statistic = BTreeMap::new();
        staker_statistic.insert(
            stash,
            online_profile::StashMachineStatistics {
                online_gpu_num: 8,
                inflation: Perbill::from_rational_approximation(8u32, 10000),
                machine_total_calc_point: 119780,
                rent_extra_grade: 0,
            },
        );
        assert_eq!(
            OnlineProfile::eras_stash_points(4),
            EraStashPoints { total: 119876, staker_statistic: staker_statistic.clone() }
        );

        let mut era_machine_points = BTreeMap::new();
        era_machine_points.insert(machine_id.clone(), MachineGradeStatus { basic_grade: 119780, is_rented: false });
        assert_eq!(OnlineProfile::eras_machine_points(4), era_machine_points);
        assert_eq!(
            OnlineProfile::user_mut_hardware_stake(&stash, &machine_id),
            online_profile::UserMutHardwareStakeInfo { ..Default::default() }
        );
        // 奖励2000DBC
        assert_eq!(
            Balances::free_balance(committee1),
            current_committee1_balance + 2000 * ONE_DBC * 333333333 / 10_0000_0000
        );
    });
}

// 三个委员会两个正常工作，一个不提交Hash值，检查惩罚机制
#[test]
fn committee_not_submit_hash_slash_works() {
    new_test_with_online_machine_distribution().execute_with(|| {
        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Alice).into();
        let committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Charlie).into();
        let _committee3: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Dave).into();
        let committee4: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();

        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();

        // 三个委员会提交Hash
        let machine_base_info = CommitteeUploadInfo {
            machine_id: machine_id.clone(),
            gpu_type: "GeForceRTX2080Ti".as_bytes().to_vec(),
            gpu_num: 4,
            cuda_core: 4352,
            gpu_mem: 11283456,
            calc_point: 6825,
            sys_disk: 12345465,
            data_disk: 324567733,
            cpu_type: "Intel(R) Xeon(R) Silver 4110 CPU".as_bytes().to_vec(),
            cpu_core_num: 32,
            cpu_rate: 26,
            mem_num: 527988672,

            rand_str: "".as_bytes().to_vec(),
            is_support: true,
        };

        let rand_str1 = "abcdefg1".as_bytes().to_vec();
        let rand_str2 = "abcdefg2".as_bytes().to_vec();

        // 委员会提交机器Hash
        let machine_info_hash1 = hex::decode("f813d5478a1c6cfb04a203a0643ad67e").unwrap().try_into().unwrap();
        let machine_info_hash2 = hex::decode("8beab87415978daf436f31a292f9bdbb").unwrap().try_into().unwrap();

        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(committee1),
            machine_id.clone(),
            machine_info_hash1,
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(committee2),
            machine_id.clone(),
            machine_info_hash2,
        ));

        let machine_committee = crate::OCMachineCommitteeList {
            book_time: 6,
            booked_committee: vec![committee2, committee1, committee4],
            hashed_committee: vec![committee2, committee1],
            confirm_start_time: 4326,
            confirmed_committee: vec![],
            onlined_committee: vec![],
            status: crate::OCVerifyStatus::default(),
        };

        assert_eq!(machine_committee, OnlineCommittee::machine_committee(&machine_id),);

        // 等到36个小时之后，提交确认信息
        run_to_block(4326); // 6 + 36 * 120 = 4326

        // 委员会提交原始信息
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            Origin::signed(committee1),
            CommitteeUploadInfo { rand_str: rand_str1, ..machine_base_info.clone() }
        ));
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            Origin::signed(committee2),
            CommitteeUploadInfo { rand_str: rand_str2, ..machine_base_info.clone() }
        ));

        run_to_block(4327);

        assert_eq!(
            OnlineProfile::live_machines(),
            online_profile::LiveMachine { online_machine: vec![machine_id.clone()], ..Default::default() }
        );

        assert_eq!(
            OnlineCommittee::pending_slash(0),
            super::OCPendingSlashInfo {
                machine_id,
                inconsistent_committee: vec![],
                unruly_committee: vec![committee4],
                reward_committee: vec![committee2, committee1],
                committee_stake: 1000 * ONE_DBC,

                slash_time: 4327,
                slash_exec_time: 4327 + 2880 * 2,

                book_result: super::OCBookResultType::OnlineSucceed,
                slash_result: super::OCSlashResult::Pending,
                ..Default::default()
            }
        );

        // 惩罚
        run_to_block(4327 + 2880 * 2 + 1);
    })
}

// 三个委员会两个正常工作，一个提交Hash之后，没有提交原始值，检查惩罚机制
#[test]
fn committee_not_wubmit_raw_slash_works() {
    new_test_with_online_machine_distribution().execute_with(|| {
        let _committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Alice).into();
        let _committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Charlie).into();
        let _committee3: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Dave).into();
        let _committee4: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();

        let _machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();
    })
}

#[test]
fn fulfill_should_works() {
    new_test_with_online_machine_distribution().execute_with(|| {})
}

// 三个委员会提交信息不一致，导致重新分派
#[test]
fn committee_not_equal_then_redistribute_works() {
    new_test_with_init_params_ext().execute_with(|| {
        // Bob pubkey
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();
        let msg = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48\
                   5CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL";
        let sig = "b4084f70730b183127e9db78c6d8dcf79039f23466cd1ee8b536c40c3027a83d\
                   ab040be4ed2db57b67eaac406817a69ce72a13f8ac11ba460e15d318b1504481";

        // 委员会需要提交的信息
        let mut committee_upload_info = CommitteeUploadInfo {
            machine_id: machine_id.clone(),
            gpu_type: "GeForceRTX3080".as_bytes().to_vec(),
            gpu_num: 4,
            cuda_core: 8704,
            gpu_mem: 10,
            calc_point: 59890,
            sys_disk: 500,
            data_disk: 3905,
            cpu_type: "Intel(R) Xeon(R) Silver 4214R".as_bytes().to_vec(),
            cpu_core_num: 46,
            cpu_rate: 2400,
            mem_num: 440,

            rand_str: "abcdefg1".as_bytes().to_vec(),
            is_support: true,
        };

        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();
        let committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One).into();
        let committee3: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Two).into();

        let committee1_box_pubkey: [u8; 32] =
            hex::decode("f660309770b2bd379e2514d88c146a7ddc3759533cf06d9fb4b41159e560325e")
                .unwrap()
                .try_into()
                .unwrap();
        let committee2_box_pubkey: [u8; 32] =
            hex::decode("9dccbab2d61405084eac440f877a6479bc827373b2e414e81a6170ebe5aadd12")
                .unwrap()
                .try_into()
                .unwrap();
        let committee3_box_pubkey: [u8; 32] =
            hex::decode("1e71b5a83ccdeff1592062a1d4da4a272691f08e2024a1ca75a81d534a76210a")
                .unwrap()
                .try_into()
                .unwrap();

        let machine_info_hash1: [u8; 16] = hex::decode("fd8885a22a9d9784adaa36effcd77522").unwrap().try_into().unwrap();
        let machine_info_hash2: [u8; 16] = hex::decode("26c58bca9792cc285aa0a2e42483131b").unwrap().try_into().unwrap();
        let machine_info_hash3: [u8; 16] = hex::decode("5745567d193b6d3cba18412489ccd433").unwrap().try_into().unwrap();

        let controller: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();
        let stash: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();

        // Machine account Info:
        // ❯ subkey generate --scheme sr25519
        //   Secret seed:       0x16f2e4b3ad50aab4f5c7ab56d793738a893080d578976040a5be284da12437b6
        //   Public key (hex):  0xdc763e931919cceee0c35392d124c753fc4e4ab6e494bc67722fdd31989d660f

        committee_upload_info.machine_id = machine_id.clone();

        // stash 账户设置控制账户
        assert_ok!(OnlineProfile::set_controller(Origin::signed(stash), controller));
        // controller 生成server_name
        assert_ok!(OnlineProfile::gen_server_room(Origin::signed(controller)));
        let server_room = OnlineProfile::stash_server_rooms(&stash);
        assert_ok!(OnlineProfile::bond_machine(
            Origin::signed(controller),
            machine_id.clone(),
            msg.as_bytes().to_vec(),
            hex::decode(sig).unwrap()
        ));

        let mut machine_info = online_profile::MachineInfo {
            controller: controller.clone(),
            machine_stash: stash.clone(),
            bonding_height: 3,
            stake_amount: 100000 * ONE_DBC,
            machine_status: online_profile::MachineStatus::AddingCustomizeInfo,
            ..Default::default()
        };

        let customize_info = StakerCustomizeInfo {
            server_room: server_room[0].clone(),
            upload_net: 100,
            download_net: 100,
            longitude: online_profile::Longitude::East(1157894),
            latitude: online_profile::Latitude::North(235678),
            telecom_operators: vec!["China Unicom".into()],
        };
        assert_ok!(OnlineProfile::add_machine_info(
            Origin::signed(controller),
            machine_id.clone(),
            customize_info.clone()
        ));

        machine_info.machine_info_detail.staker_customize_info = customize_info.clone();
        machine_info.machine_status = online_profile::MachineStatus::DistributingOrder;

        run_to_block(15);

        // 添加三个委员会
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee1));
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee2));
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee3));

        assert_ok!(Committee::committee_set_box_pubkey(Origin::signed(committee1), committee1_box_pubkey.clone()));
        assert_ok!(Committee::committee_set_box_pubkey(Origin::signed(committee2), committee2_box_pubkey.clone()));
        assert_ok!(Committee::committee_set_box_pubkey(Origin::signed(committee3), committee3_box_pubkey.clone()));

        run_to_block(16);
        assert_eq!(
            OnlineProfile::live_machines(),
            online_profile::LiveMachine { booked_machine: vec![machine_id.clone()], ..Default::default() }
        );

        machine_info.machine_status = online_profile::MachineStatus::CommitteeVerifying;

        // 正常Hash: 0x6b561dfad171810dfb69924dd68733ec
        // cpu_core_num: 48: 0x5b4499c4b6e9f080673f9573410a103a
        // cpu_core_num: 96: 0x3ac5b3416d1743b58a4c9af58c7002d7

        // 三个委员会分别提交机器Hash
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(committee1),
            machine_id.clone(),
            machine_info_hash1
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(committee2),
            machine_id.clone(),
            machine_info_hash2
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(committee3),
            machine_id.clone(),
            machine_info_hash3
        ));

        // 委员会提交原始信息
        assert_ok!(OnlineCommittee::submit_confirm_raw(Origin::signed(committee1), committee_upload_info.clone()));
        committee_upload_info.mem_num = 441;
        assert_ok!(OnlineCommittee::submit_confirm_raw(Origin::signed(committee2), committee_upload_info.clone()));
        committee_upload_info.mem_num = 442;
        assert_ok!(OnlineCommittee::submit_confirm_raw(Origin::signed(committee3), committee_upload_info.clone()));

        assert_eq!(
            OnlineCommittee::machine_committee(&machine_id),
            super::OCMachineCommitteeList {
                book_time: 16,
                booked_committee: vec![committee3, committee1, committee2],
                hashed_committee: vec![committee3, committee1, committee2],
                confirmed_committee: vec![committee3, committee1, committee2],
                confirm_start_time: 4336,
                status: super::OCVerifyStatus::Summarizing,
                ..Default::default()
            }
        );

        assert_eq!(
            Committee::committee_stake(committee1),
            committee::CommitteeStakeInfo {
                box_pubkey: committee1_box_pubkey,
                staked_amount: 20000 * ONE_DBC,
                used_stake: 1000 * ONE_DBC,
                ..Default::default()
            }
        );
        assert_eq!(
            Committee::committee(),
            committee::CommitteeList { normal: vec![committee3, committee1, committee2], ..Default::default() }
        );

        run_to_block(17);

        // 机器直接删掉信息即可
        assert_eq!(
            OnlineCommittee::committee_machine(&committee1),
            OCCommitteeMachineList { booked_machine: vec![machine_id.clone()], ..Default::default() }
        );
        assert_eq!(
            OnlineCommittee::committee_machine(&committee2),
            OCCommitteeMachineList { booked_machine: vec![machine_id.clone()], ..Default::default() }
        );
        assert_eq!(
            OnlineCommittee::committee_machine(&committee3),
            OCCommitteeMachineList { booked_machine: vec![machine_id.clone()], ..Default::default() }
        );

        // 如果on_finalize先执行lease_committee 再z执行online_profile则没有内容，否则被重新分配了
        assert_eq!(
            OnlineCommittee::machine_committee(&machine_id),
            super::OCMachineCommitteeList {
                book_time: 17,
                booked_committee: vec![committee3, committee1, committee2],
                confirm_start_time: 4337,
                ..Default::default()
            }
        );

        let machine_submit_hash: Vec<[u8; 16]> = vec![];
        assert_eq!(OnlineCommittee::machine_submited_hash(&machine_id), machine_submit_hash);
        assert_eq!(
            OnlineCommittee::committee_ops(&committee1, &machine_id),
            super::OCCommitteeOps {
                staked_dbc: 1000 * ONE_DBC,
                verify_time: vec![497, 1937, 3377],
                ..Default::default()
            }
        );
    })
}

// NOTE: 测试summary函数
// 当全部提交Hash+全部提交原始值时:
// case 1: 3个支持，内容一致 ->上线
// case 2: 3个支持，2内容一致 -> 上线 + 惩罚
// case 3: 2个支持，1个反对 (2个一致) -> 上线 + 惩罚
// case 4: 3个支持，内容都不一致 -> 无共识 + 重新分配
// case 5: 2个支持，1个反对（2个不一致） -> 无共识 + 重新分配
// case 6: 2个反对，1个支持 -> 不上线 + 奖励 + 惩罚
// case 7: 3个反对 -> 不上线 + 奖励
// case 8: 2提交Hash， 2提交原始值，都是反对
// case 9: 2提交Hash， 2提交原始值，都是支持
// case 10: 全部提交Hash，2提交原始值，且都是支持，两个不相同
// case 11: 全部提交Hash，2提交原始值时，且都是支持，两个相同

// case 1: 3个支持，内容一致 ->上线
#[test]
fn test_summary_confirmation1() {
    new_test_with_init_params_ext().execute_with(|| {
        let machine_id = "484f457327950359de97c4b4c193bb3c8ddbe1dce56f038b3ac2b90e40995241".as_bytes().to_vec();

        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One).into();
        let committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Two).into();
        let committee3: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();

        run_to_block(10);

        // 构建 machine_committee
        <super::MachineCommittee<TestRuntime>>::insert(
            &machine_id,
            super::OCMachineCommitteeList {
                book_time: 9,
                booked_committee: vec![committee2, committee3, committee1],
                hashed_committee: vec![committee2, committee3, committee1],
                confirm_start_time: 5432,
                confirmed_committee: vec![committee2, committee3, committee1],
                onlined_committee: vec![],
                status: super::OCVerifyStatus::Summarizing,
            },
        );

        let machine_info_hash: [u8; 16] = hex::decode("d80b116fd318f19fd89da792aba5e875").unwrap().try_into().unwrap();

        let committee_ops = super::OCCommitteeOps {
            staked_dbc: 1000 * ONE_DBC,
            verify_time: vec![],
            confirm_hash: machine_info_hash.clone(),
            hash_time: 16887,
            confirm_time: 16891,
            machine_status: super::OCMachineStatus::Confirmed,
            machine_info: super::CommitteeUploadInfo {
                machine_id: "484f457327950359de97c4b4c193bb3c8ddbe1dce56f038b3ac2b90e40995241".as_bytes().to_vec(),
                gpu_type: "GeForceRTX3060".as_bytes().to_vec(),
                gpu_num: 4,
                cuda_core: 3584,
                gpu_mem: 12,
                calc_point: 41718,
                sys_disk: 256,
                data_disk: 1800,
                cpu_type: "Intel(R) Xeon(R) Platinum Intel 8259L CPU".as_bytes().to_vec(),
                cpu_core_num: 96,
                cpu_rate: 2400,
                mem_num: 192,
                rand_str: "".as_bytes().to_vec(),
                is_support: true,
            },
        };

        let committee1_ops = super::OCCommitteeOps {
            verify_time: vec![1622, 3062, 4502],
            confirm_hash: machine_info_hash.clone(),
            ..committee_ops.clone()
        };

        let committee2_ops = super::OCCommitteeOps {
            verify_time: vec![1142, 2582, 4022],
            confirm_hash: machine_info_hash.clone(),
            ..committee_ops.clone()
        };

        let committee3_ops = super::OCCommitteeOps {
            verify_time: vec![662, 2102, 3542],
            confirm_hash: machine_info_hash.clone(),
            ..committee_ops.clone()
        };

        // 构建committee_ops
        <CommitteeOps<TestRuntime>>::insert(&committee1, &machine_id, committee1_ops);
        <CommitteeOps<TestRuntime>>::insert(&committee2, &machine_id, committee2_ops);
        <CommitteeOps<TestRuntime>>::insert(&committee3, &machine_id, committee3_ops);

        let summary = super::Summary {
            valid_support: vec![committee2, committee3, committee1],
            info: Some(committee_ops.machine_info.clone()),
            ..Default::default()
        };

        assert_eq!(
            OnlineCommittee::summary_confirmation(&machine_id),
            super::MachineConfirmStatus::Confirmed(summary.clone())
        );
    })
}

// case 2: 3个支持，2内容一致 -> 上线 + 惩罚
#[test]
fn test_summary_confirmation2() {
    new_test_with_init_params_ext().execute_with(|| {
        let machine_id = "484f457327950359de97c4b4c193bb3c8ddbe1dce56f038b3ac2b90e40995241".as_bytes().to_vec();

        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One).into();
        let committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Two).into();
        let committee3: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();

        run_to_block(10);

        // 构建 machine_committee
        <super::MachineCommittee<TestRuntime>>::insert(
            &machine_id,
            super::OCMachineCommitteeList {
                book_time: 9,
                booked_committee: vec![committee2, committee3, committee1],
                hashed_committee: vec![committee2, committee3, committee1],
                confirm_start_time: 5432,
                confirmed_committee: vec![committee2, committee3, committee1],
                onlined_committee: vec![],
                status: super::OCVerifyStatus::Summarizing,
            },
        );

        let machine_info_hash: [u8; 16] = hex::decode("d80b116fd318f19fd89da792aba5e875").unwrap().try_into().unwrap();

        let committee_ops = super::OCCommitteeOps {
            staked_dbc: 1000 * ONE_DBC,
            verify_time: vec![],
            confirm_hash: machine_info_hash.clone(),
            hash_time: 16887,
            confirm_time: 16891,
            machine_status: super::OCMachineStatus::Confirmed,
            machine_info: super::CommitteeUploadInfo {
                machine_id: "484f457327950359de97c4b4c193bb3c8ddbe1dce56f038b3ac2b90e40995241".as_bytes().to_vec(),
                gpu_type: "GeForceRTX3060".as_bytes().to_vec(),
                gpu_num: 4,
                cuda_core: 3584,
                gpu_mem: 12,
                calc_point: 41718,
                sys_disk: 256,
                data_disk: 1800,
                cpu_type: "Intel(R) Xeon(R) Platinum Intel 8259L CPU".as_bytes().to_vec(),
                cpu_core_num: 96,
                cpu_rate: 2400,
                mem_num: 192,
                rand_str: "".as_bytes().to_vec(),
                is_support: true,
            },
        };

        let committee1_ops = super::OCCommitteeOps {
            verify_time: vec![1622, 3062, 4502],
            confirm_hash: machine_info_hash.clone(),
            ..committee_ops.clone()
        };

        let committee2_ops = super::OCCommitteeOps {
            verify_time: vec![1142, 2582, 4022],
            confirm_hash: machine_info_hash.clone(),
            ..committee_ops.clone()
        };

        let committee3_ops = super::OCCommitteeOps {
            verify_time: vec![662, 2102, 3542],
            confirm_hash: machine_info_hash.clone(),
            machine_info: super::CommitteeUploadInfo { gpu_num: 3, ..committee_ops.machine_info.clone() },
            ..committee_ops.clone()
        };

        // 构建committee_ops
        <CommitteeOps<TestRuntime>>::insert(&committee1, &machine_id, committee1_ops);
        <CommitteeOps<TestRuntime>>::insert(&committee2, &machine_id, committee2_ops);
        <CommitteeOps<TestRuntime>>::insert(&committee3, &machine_id, committee3_ops);

        let summary = super::Summary {
            valid_support: vec![committee2, committee1],
            invalid_support: vec![committee3],
            info: Some(committee_ops.machine_info.clone()),
            ..Default::default()
        };

        assert_eq!(
            OnlineCommittee::summary_confirmation(&machine_id),
            super::MachineConfirmStatus::Confirmed(summary.clone())
        );
    })
}

// case 3: 2个支持，1个反对 (2个一致) -> 上线 + 惩罚
#[test]
fn test_summary_confirmation3() {
    new_test_with_init_params_ext().execute_with(|| {
        let machine_id = "484f457327950359de97c4b4c193bb3c8ddbe1dce56f038b3ac2b90e40995241".as_bytes().to_vec();

        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One).into();
        let committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Two).into();
        let committee3: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();

        run_to_block(10);

        // 构建 machine_committee
        <super::MachineCommittee<TestRuntime>>::insert(
            &machine_id,
            super::OCMachineCommitteeList {
                book_time: 9,
                booked_committee: vec![committee2, committee3, committee1],
                hashed_committee: vec![committee2, committee3, committee1],
                confirm_start_time: 5432,
                confirmed_committee: vec![committee2, committee3, committee1],
                onlined_committee: vec![],
                status: super::OCVerifyStatus::Summarizing,
            },
        );

        let machine_info_hash: [u8; 16] = hex::decode("d80b116fd318f19fd89da792aba5e875").unwrap().try_into().unwrap();

        let committee_ops = super::OCCommitteeOps {
            staked_dbc: 1000 * ONE_DBC,
            verify_time: vec![],
            confirm_hash: machine_info_hash.clone(),
            hash_time: 16887,
            confirm_time: 16891,
            machine_status: super::OCMachineStatus::Confirmed,
            machine_info: super::CommitteeUploadInfo {
                machine_id: "484f457327950359de97c4b4c193bb3c8ddbe1dce56f038b3ac2b90e40995241".as_bytes().to_vec(),
                gpu_type: "GeForceRTX3060".as_bytes().to_vec(),
                gpu_num: 4,
                cuda_core: 3584,
                gpu_mem: 12,
                calc_point: 41718,
                sys_disk: 256,
                data_disk: 1800,
                cpu_type: "Intel(R) Xeon(R) Platinum Intel 8259L CPU".as_bytes().to_vec(),
                cpu_core_num: 96,
                cpu_rate: 2400,
                mem_num: 192,
                rand_str: "".as_bytes().to_vec(),
                is_support: true,
            },
        };

        let committee1_ops = super::OCCommitteeOps {
            verify_time: vec![1622, 3062, 4502],
            confirm_hash: machine_info_hash.clone(),
            ..committee_ops.clone()
        };

        let committee2_ops = super::OCCommitteeOps {
            verify_time: vec![1142, 2582, 4022],
            confirm_hash: machine_info_hash.clone(),
            ..committee_ops.clone()
        };

        let committee3_ops = super::OCCommitteeOps {
            verify_time: vec![662, 2102, 3542],
            confirm_hash: machine_info_hash.clone(),
            machine_info: super::CommitteeUploadInfo {
                gpu_num: 3,
                is_support: false,
                ..committee_ops.machine_info.clone()
            },
            ..committee_ops.clone()
        };

        // 构建committee_ops
        CommitteeOps::<TestRuntime>::insert(&committee1, &machine_id, committee1_ops);
        CommitteeOps::<TestRuntime>::insert(&committee2, &machine_id, committee2_ops);
        CommitteeOps::<TestRuntime>::insert(&committee3, &machine_id, committee3_ops);

        let summary = super::Summary {
            valid_support: vec![committee2, committee1],
            against: vec![committee3],
            info: Some(committee_ops.machine_info.clone()),
            ..Default::default()
        };

        assert_eq!(
            OnlineCommittee::summary_confirmation(&machine_id),
            super::MachineConfirmStatus::Confirmed(summary.clone())
        );
    })
}

// case 4: 3个支持，内容都不一致 -> 无共识 + 重新分配
#[test]
fn test_summary_confirmation4() {
    new_test_with_init_params_ext().execute_with(|| {
        let machine_id = "484f457327950359de97c4b4c193bb3c8ddbe1dce56f038b3ac2b90e40995241".as_bytes().to_vec();

        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One).into();
        let committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Two).into();
        let committee3: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();

        run_to_block(10);

        // 构建 machine_committee
        <super::MachineCommittee<TestRuntime>>::insert(
            &machine_id,
            super::OCMachineCommitteeList {
                book_time: 9,
                booked_committee: vec![committee2, committee3, committee1],
                hashed_committee: vec![committee2, committee3, committee1],
                confirm_start_time: 5432,
                confirmed_committee: vec![committee2, committee3, committee1],
                onlined_committee: vec![],
                status: super::OCVerifyStatus::Summarizing,
            },
        );

        let machine_info_hash: [u8; 16] = hex::decode("d80b116fd318f19fd89da792aba5e875").unwrap().try_into().unwrap();

        let committee_ops = super::OCCommitteeOps {
            staked_dbc: 1000 * ONE_DBC,
            verify_time: vec![],
            confirm_hash: machine_info_hash.clone(),
            hash_time: 16887,
            confirm_time: 16891,
            machine_status: super::OCMachineStatus::Confirmed,
            machine_info: super::CommitteeUploadInfo {
                machine_id: "484f457327950359de97c4b4c193bb3c8ddbe1dce56f038b3ac2b90e40995241".as_bytes().to_vec(),
                gpu_type: "GeForceRTX3060".as_bytes().to_vec(),
                gpu_num: 4,
                cuda_core: 3584,
                gpu_mem: 12,
                calc_point: 41718,
                sys_disk: 256,
                data_disk: 1800,
                cpu_type: "Intel(R) Xeon(R) Platinum Intel 8259L CPU".as_bytes().to_vec(),
                cpu_core_num: 96,
                cpu_rate: 2400,
                mem_num: 192,
                rand_str: "".as_bytes().to_vec(),
                is_support: true,
            },
        };

        let committee1_ops = super::OCCommitteeOps {
            verify_time: vec![1622, 3062, 4502],
            confirm_hash: machine_info_hash.clone(),
            ..committee_ops.clone()
        };

        let committee2_ops = super::OCCommitteeOps {
            verify_time: vec![1142, 2582, 4022],
            confirm_hash: machine_info_hash.clone(),

            machine_info: super::CommitteeUploadInfo { gpu_num: 5, ..committee_ops.machine_info.clone() },
            ..committee_ops.clone()
        };

        let committee3_ops = super::OCCommitteeOps {
            verify_time: vec![662, 2102, 3542],
            confirm_hash: machine_info_hash.clone(),
            machine_info: super::CommitteeUploadInfo { gpu_num: 3, ..committee_ops.machine_info.clone() },
            ..committee_ops.clone()
        };

        // 构建committee_ops
        CommitteeOps::<TestRuntime>::insert(&committee1, &machine_id, committee1_ops);
        CommitteeOps::<TestRuntime>::insert(&committee2, &machine_id, committee2_ops);
        CommitteeOps::<TestRuntime>::insert(&committee3, &machine_id, committee3_ops);

        assert_eq!(
            OnlineCommittee::summary_confirmation(&machine_id),
            super::MachineConfirmStatus::NoConsensus(super::Summary {
                invalid_support: vec![committee2, committee3, committee1],
                ..Default::default()
            }),
        );
    })
}

// case 5: 2个支持，1个反对（2个不一致） -> 无共识 + 重新分配
#[test]
fn test_summary_confirmation5() {
    new_test_with_init_params_ext().execute_with(|| {
        let machine_id = "484f457327950359de97c4b4c193bb3c8ddbe1dce56f038b3ac2b90e40995241".as_bytes().to_vec();

        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One).into();
        let committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Two).into();
        let committee3: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();

        run_to_block(10);

        // 构建 machine_committee
        <super::MachineCommittee<TestRuntime>>::insert(
            &machine_id,
            super::OCMachineCommitteeList {
                book_time: 9,
                booked_committee: vec![committee2, committee3, committee1],
                hashed_committee: vec![committee2, committee3, committee1],
                confirm_start_time: 5432,
                confirmed_committee: vec![committee2, committee3, committee1],
                onlined_committee: vec![],
                status: super::OCVerifyStatus::Summarizing,
            },
        );

        let machine_info_hash: [u8; 16] = hex::decode("d80b116fd318f19fd89da792aba5e875").unwrap().try_into().unwrap();

        let committee_ops = super::OCCommitteeOps {
            staked_dbc: 1000 * ONE_DBC,
            verify_time: vec![],
            confirm_hash: machine_info_hash.clone(),
            hash_time: 16887,
            confirm_time: 16891,
            machine_status: super::OCMachineStatus::Confirmed,
            machine_info: super::CommitteeUploadInfo {
                machine_id: "484f457327950359de97c4b4c193bb3c8ddbe1dce56f038b3ac2b90e40995241".as_bytes().to_vec(),
                gpu_type: "GeForceRTX3060".as_bytes().to_vec(),
                gpu_num: 4,
                cuda_core: 3584,
                gpu_mem: 12,
                calc_point: 41718,
                sys_disk: 256,
                data_disk: 1800,
                cpu_type: "Intel(R) Xeon(R) Platinum Intel 8259L CPU".as_bytes().to_vec(),
                cpu_core_num: 96,
                cpu_rate: 2400,
                mem_num: 192,
                rand_str: "".as_bytes().to_vec(),
                is_support: true,
            },
        };

        let committee1_ops = super::OCCommitteeOps {
            verify_time: vec![1622, 3062, 4502],
            confirm_hash: machine_info_hash.clone(),
            machine_info: super::CommitteeUploadInfo { gpu_num: 4, ..committee_ops.machine_info.clone() },

            ..committee_ops.clone()
        };

        let committee2_ops = super::OCCommitteeOps {
            verify_time: vec![1142, 2582, 4022],
            confirm_hash: machine_info_hash.clone(),
            machine_info: super::CommitteeUploadInfo { gpu_num: 3, ..committee_ops.machine_info.clone() },

            ..committee_ops.clone()
        };

        let committee3_ops = super::OCCommitteeOps {
            verify_time: vec![662, 2102, 3542],
            confirm_hash: machine_info_hash.clone(),
            machine_info: super::CommitteeUploadInfo {
                gpu_num: 3,
                is_support: false,
                ..committee_ops.machine_info.clone()
            },
            ..committee_ops.clone()
        };

        // 构建committee_ops
        CommitteeOps::<TestRuntime>::insert(&committee1, &machine_id, committee1_ops);
        CommitteeOps::<TestRuntime>::insert(&committee2, &machine_id, committee2_ops);
        CommitteeOps::<TestRuntime>::insert(&committee3, &machine_id, committee3_ops);

        let summary = super::Summary {
            invalid_support: vec![committee2, committee1],
            against: vec![committee3],
            ..Default::default()
        };

        assert_eq!(
            OnlineCommittee::summary_confirmation(&machine_id),
            super::MachineConfirmStatus::NoConsensus(summary.clone())
        );
    })
}

// case 6: 2个反对，1个支持 -> 不上线 + 奖励 + 惩罚
#[test]
fn test_summary_confirmation6() {
    new_test_with_init_params_ext().execute_with(|| {
        let machine_id = "484f457327950359de97c4b4c193bb3c8ddbe1dce56f038b3ac2b90e40995241".as_bytes().to_vec();

        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One).into();
        let committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Two).into();
        let committee3: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();

        run_to_block(10);

        // 构建 machine_committee
        <super::MachineCommittee<TestRuntime>>::insert(
            &machine_id,
            super::OCMachineCommitteeList {
                book_time: 9,
                booked_committee: vec![committee2, committee3, committee1],
                hashed_committee: vec![committee2, committee3, committee1],
                confirm_start_time: 5432,
                confirmed_committee: vec![committee2, committee3, committee1],
                onlined_committee: vec![],
                status: super::OCVerifyStatus::Summarizing,
            },
        );

        let machine_info_hash: [u8; 16] = hex::decode("d80b116fd318f19fd89da792aba5e875").unwrap().try_into().unwrap();

        let committee_ops = super::OCCommitteeOps {
            staked_dbc: 1000 * ONE_DBC,
            verify_time: vec![],
            confirm_hash: machine_info_hash.clone(),
            hash_time: 16887,
            confirm_time: 16891,
            machine_status: super::OCMachineStatus::Confirmed,
            machine_info: super::CommitteeUploadInfo {
                machine_id: "484f457327950359de97c4b4c193bb3c8ddbe1dce56f038b3ac2b90e40995241".as_bytes().to_vec(),
                gpu_type: "GeForceRTX3060".as_bytes().to_vec(),
                gpu_num: 4,
                cuda_core: 3584,
                gpu_mem: 12,
                calc_point: 41718,
                sys_disk: 256,
                data_disk: 1800,
                cpu_type: "Intel(R) Xeon(R) Platinum Intel 8259L CPU".as_bytes().to_vec(),
                cpu_core_num: 96,
                cpu_rate: 2400,
                mem_num: 192,
                rand_str: "".as_bytes().to_vec(),
                is_support: true,
            },
        };

        let committee1_ops = super::OCCommitteeOps {
            verify_time: vec![1622, 3062, 4502],
            confirm_hash: machine_info_hash.clone(),
            machine_info: super::CommitteeUploadInfo {
                gpu_num: 3,
                is_support: false,
                ..committee_ops.machine_info.clone()
            },
            ..committee_ops.clone()
        };

        let committee2_ops = super::OCCommitteeOps {
            verify_time: vec![1142, 2582, 4022],
            confirm_hash: machine_info_hash.clone(),
            machine_info: super::CommitteeUploadInfo { gpu_num: 3, ..committee_ops.machine_info.clone() },
            ..committee_ops.clone()
        };

        let committee3_ops = super::OCCommitteeOps {
            verify_time: vec![662, 2102, 3542],
            confirm_hash: machine_info_hash.clone(),
            machine_info: super::CommitteeUploadInfo {
                gpu_num: 3,
                is_support: false,
                ..committee_ops.machine_info.clone()
            },
            ..committee_ops.clone()
        };

        // 构建committee_ops
        CommitteeOps::<TestRuntime>::insert(&committee1, &machine_id, committee1_ops);
        CommitteeOps::<TestRuntime>::insert(&committee2, &machine_id, committee2_ops);
        CommitteeOps::<TestRuntime>::insert(&committee3, &machine_id, committee3_ops);

        let summary = super::Summary {
            invalid_support: vec![committee2],
            against: vec![committee3, committee1],
            ..Default::default()
        };

        assert_eq!(
            OnlineCommittee::summary_confirmation(&machine_id),
            super::MachineConfirmStatus::Refuse(summary.clone())
        );
    })
}

// case 7: 3个反对 -> 不上线 + 奖励
#[test]
fn test_summary_confirmation7() {
    new_test_with_init_params_ext().execute_with(|| {
        let machine_id = "484f457327950359de97c4b4c193bb3c8ddbe1dce56f038b3ac2b90e40995241".as_bytes().to_vec();

        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One).into();
        let committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Two).into();
        let committee3: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();

        run_to_block(10);

        // 构建 machine_committee
        <super::MachineCommittee<TestRuntime>>::insert(
            &machine_id,
            super::OCMachineCommitteeList {
                book_time: 9,
                booked_committee: vec![committee2, committee3, committee1],
                hashed_committee: vec![committee2, committee3, committee1],
                confirm_start_time: 5432,
                confirmed_committee: vec![committee2, committee3, committee1],
                onlined_committee: vec![],
                status: super::OCVerifyStatus::Summarizing,
            },
        );

        let machine_info_hash: [u8; 16] = hex::decode("d80b116fd318f19fd89da792aba5e875").unwrap().try_into().unwrap();

        let committee_ops = super::OCCommitteeOps {
            staked_dbc: 1000 * ONE_DBC,
            verify_time: vec![],
            confirm_hash: machine_info_hash.clone(),
            hash_time: 16887,
            confirm_time: 16891,
            machine_status: super::OCMachineStatus::Confirmed,
            machine_info: super::CommitteeUploadInfo {
                machine_id: "484f457327950359de97c4b4c193bb3c8ddbe1dce56f038b3ac2b90e40995241".as_bytes().to_vec(),
                gpu_type: "GeForceRTX3060".as_bytes().to_vec(),
                gpu_num: 4,
                cuda_core: 3584,
                gpu_mem: 12,
                calc_point: 41718,
                sys_disk: 256,
                data_disk: 1800,
                cpu_type: "Intel(R) Xeon(R) Platinum Intel 8259L CPU".as_bytes().to_vec(),
                cpu_core_num: 96,
                cpu_rate: 2400,
                mem_num: 192,
                rand_str: "".as_bytes().to_vec(),
                is_support: true,
            },
        };

        let committee1_ops = super::OCCommitteeOps {
            verify_time: vec![1622, 3062, 4502],
            confirm_hash: machine_info_hash.clone(),
            machine_info: super::CommitteeUploadInfo {
                gpu_num: 3,
                is_support: false,
                ..committee_ops.machine_info.clone()
            },
            ..committee_ops.clone()
        };

        let committee2_ops = super::OCCommitteeOps {
            verify_time: vec![1142, 2582, 4022],
            confirm_hash: machine_info_hash.clone(),
            machine_info: super::CommitteeUploadInfo {
                gpu_num: 3,
                is_support: false,
                ..committee_ops.machine_info.clone()
            },
            ..committee_ops.clone()
        };

        let committee3_ops = super::OCCommitteeOps {
            verify_time: vec![662, 2102, 3542],
            confirm_hash: machine_info_hash.clone(),
            machine_info: super::CommitteeUploadInfo {
                gpu_num: 3,
                is_support: false,
                ..committee_ops.machine_info.clone()
            },
            ..committee_ops.clone()
        };

        // 构建committee_ops
        CommitteeOps::<TestRuntime>::insert(&committee1, &machine_id, committee1_ops);
        CommitteeOps::<TestRuntime>::insert(&committee2, &machine_id, committee2_ops);
        CommitteeOps::<TestRuntime>::insert(&committee3, &machine_id, committee3_ops);

        let summary = super::Summary { against: vec![committee2, committee3, committee1], ..Default::default() };

        assert_eq!(
            OnlineCommittee::summary_confirmation(&machine_id),
            super::MachineConfirmStatus::Refuse(summary.clone())
        );
    })
}

// case 8: 2提交Hash， 2提交原始值，且都是反对
#[test]
fn test_summary_confirmation8() {
    new_test_with_init_params_ext().execute_with(|| {
        let machine_id = "484f457327950359de97c4b4c193bb3c8ddbe1dce56f038b3ac2b90e40995241".as_bytes().to_vec();

        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One).into();
        let committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Two).into();
        let committee3: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();

        run_to_block(10);

        // 构建 machine_committee
        <super::MachineCommittee<TestRuntime>>::insert(
            &machine_id,
            super::OCMachineCommitteeList {
                book_time: 9,
                booked_committee: vec![committee2, committee3, committee1],
                hashed_committee: vec![committee2, committee3],
                confirm_start_time: 5432,
                confirmed_committee: vec![committee2, committee3],
                onlined_committee: vec![],
                status: super::OCVerifyStatus::Summarizing,
            },
        );

        let machine_info_hash: [u8; 16] = hex::decode("d80b116fd318f19fd89da792aba5e875").unwrap().try_into().unwrap();

        let committee_ops = super::OCCommitteeOps {
            staked_dbc: 1000 * ONE_DBC,
            verify_time: vec![],
            confirm_hash: machine_info_hash.clone(),
            hash_time: 16887,
            confirm_time: 16891,
            machine_status: super::OCMachineStatus::Confirmed,
            machine_info: super::CommitteeUploadInfo {
                machine_id: "484f457327950359de97c4b4c193bb3c8ddbe1dce56f038b3ac2b90e40995241".as_bytes().to_vec(),
                gpu_type: "GeForceRTX3060".as_bytes().to_vec(),
                gpu_num: 4,
                cuda_core: 3584,
                gpu_mem: 12,
                calc_point: 41718,
                sys_disk: 256,
                data_disk: 1800,
                cpu_type: "Intel(R) Xeon(R) Platinum Intel 8259L CPU".as_bytes().to_vec(),
                cpu_core_num: 96,
                cpu_rate: 2400,
                mem_num: 192,
                rand_str: "".as_bytes().to_vec(),
                is_support: true,
            },
        };

        let committee2_ops = super::OCCommitteeOps {
            verify_time: vec![1142, 2582, 4022],
            confirm_hash: machine_info_hash.clone(),
            machine_info: super::CommitteeUploadInfo {
                gpu_num: 3,
                is_support: false,
                ..committee_ops.machine_info.clone()
            },
            ..committee_ops.clone()
        };

        let committee3_ops = super::OCCommitteeOps {
            verify_time: vec![662, 2102, 3542],
            confirm_hash: machine_info_hash.clone(),
            machine_info: super::CommitteeUploadInfo {
                gpu_num: 3,
                is_support: false,
                ..committee_ops.machine_info.clone()
            },
            ..committee_ops.clone()
        };

        // 构建committee_ops
        CommitteeOps::<TestRuntime>::insert(&committee2, &machine_id, committee2_ops);
        CommitteeOps::<TestRuntime>::insert(&committee3, &machine_id, committee3_ops);

        let summary =
            super::Summary { unruly: vec![committee1], against: vec![committee2, committee3], ..Default::default() };

        assert_eq!(
            OnlineCommittee::summary_confirmation(&machine_id),
            super::MachineConfirmStatus::Refuse(summary.clone())
        );
    })
}

// case 9: 2提交Hash，2提交原始值，且都是支持
#[test]
fn test_summary_confirmation9() {
    new_test_with_init_params_ext().execute_with(|| {
        let machine_id = "484f457327950359de97c4b4c193bb3c8ddbe1dce56f038b3ac2b90e40995241".as_bytes().to_vec();

        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One).into();
        let committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Two).into();
        let committee3: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();

        run_to_block(10);

        // 构建 machine_committee
        <super::MachineCommittee<TestRuntime>>::insert(
            &machine_id,
            super::OCMachineCommitteeList {
                book_time: 9,
                booked_committee: vec![committee2, committee3, committee1],
                hashed_committee: vec![committee2, committee3],
                confirm_start_time: 5432,
                confirmed_committee: vec![committee2, committee3],
                onlined_committee: vec![],
                status: super::OCVerifyStatus::Summarizing,
            },
        );

        let machine_info_hash: [u8; 16] = hex::decode("d80b116fd318f19fd89da792aba5e875").unwrap().try_into().unwrap();

        let committee_ops = super::OCCommitteeOps {
            staked_dbc: 1000 * ONE_DBC,
            verify_time: vec![],
            confirm_hash: machine_info_hash.clone(),
            hash_time: 16887,
            confirm_time: 16891,
            machine_status: super::OCMachineStatus::Confirmed,
            machine_info: super::CommitteeUploadInfo {
                machine_id: "484f457327950359de97c4b4c193bb3c8ddbe1dce56f038b3ac2b90e40995241".as_bytes().to_vec(),
                gpu_type: "GeForceRTX3060".as_bytes().to_vec(),
                gpu_num: 4,
                cuda_core: 3584,
                gpu_mem: 12,
                calc_point: 41718,
                sys_disk: 256,
                data_disk: 1800,
                cpu_type: "Intel(R) Xeon(R) Platinum Intel 8259L CPU".as_bytes().to_vec(),
                cpu_core_num: 96,
                cpu_rate: 2400,
                mem_num: 192,
                rand_str: "".as_bytes().to_vec(),
                is_support: true,
            },
        };

        let committee2_ops = super::OCCommitteeOps {
            verify_time: vec![1142, 2582, 4022],
            confirm_hash: machine_info_hash.clone(),
            ..committee_ops.clone()
        };

        let committee3_ops = super::OCCommitteeOps {
            verify_time: vec![662, 2102, 3542],
            confirm_hash: machine_info_hash.clone(),
            ..committee_ops.clone()
        };

        // 构建committee_ops
        <CommitteeOps<TestRuntime>>::insert(&committee2, &machine_id, committee2_ops);
        <CommitteeOps<TestRuntime>>::insert(&committee3, &machine_id, committee3_ops);

        let summary = super::Summary {
            valid_support: vec![committee2, committee3],
            unruly: vec![committee1],
            info: Some(committee_ops.machine_info.clone()),
            ..Default::default()
        };

        assert_eq!(
            OnlineCommittee::summary_confirmation(&machine_id),
            super::MachineConfirmStatus::Confirmed(summary.clone())
        );
    })
}

// case 10: 3提交Hash，2提交原始值，且都是支持，且两个互不相等
#[test]
fn test_summary_confirmation10() {
    new_test_with_init_params_ext().execute_with(|| {
        let machine_id = "484f457327950359de97c4b4c193bb3c8ddbe1dce56f038b3ac2b90e40995241".as_bytes().to_vec();

        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One).into();
        let committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Two).into();
        let committee3: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();

        run_to_block(10);

        // 构建 machine_committee
        <super::MachineCommittee<TestRuntime>>::insert(
            &machine_id,
            super::OCMachineCommitteeList {
                book_time: 9,
                booked_committee: vec![committee2, committee3, committee1],
                hashed_committee: vec![committee2, committee3],
                confirm_start_time: 5432,
                confirmed_committee: vec![committee2, committee3],
                onlined_committee: vec![],
                status: super::OCVerifyStatus::Summarizing,
            },
        );

        let machine_info_hash: [u8; 16] = hex::decode("d80b116fd318f19fd89da792aba5e875").unwrap().try_into().unwrap();

        let committee_ops = super::OCCommitteeOps {
            staked_dbc: 1000 * ONE_DBC,
            verify_time: vec![],
            confirm_hash: machine_info_hash.clone(),
            hash_time: 16887,
            confirm_time: 16891,
            machine_status: super::OCMachineStatus::Confirmed,
            machine_info: super::CommitteeUploadInfo {
                machine_id: "484f457327950359de97c4b4c193bb3c8ddbe1dce56f038b3ac2b90e40995241".as_bytes().to_vec(),
                gpu_type: "GeForceRTX3060".as_bytes().to_vec(),
                gpu_num: 4,
                cuda_core: 3584,
                gpu_mem: 12,
                calc_point: 41718,
                sys_disk: 256,
                data_disk: 1800,
                cpu_type: "Intel(R) Xeon(R) Platinum Intel 8259L CPU".as_bytes().to_vec(),
                cpu_core_num: 96,
                cpu_rate: 2400,
                mem_num: 192,
                rand_str: "".as_bytes().to_vec(),
                is_support: true,
            },
        };

        let committee2_ops = super::OCCommitteeOps {
            verify_time: vec![1142, 2582, 4022],
            confirm_hash: machine_info_hash.clone(),
            machine_info: super::CommitteeUploadInfo { gpu_num: 3, ..committee_ops.machine_info.clone() },
            ..committee_ops.clone()
        };

        let committee3_ops = super::OCCommitteeOps {
            verify_time: vec![662, 2102, 3542],
            confirm_hash: machine_info_hash.clone(),
            ..committee_ops.clone()
        };

        // 构建committee_ops
        <CommitteeOps<TestRuntime>>::insert(&committee2, &machine_id, committee2_ops);
        <CommitteeOps<TestRuntime>>::insert(&committee3, &machine_id, committee3_ops);

        assert_eq!(
            OnlineCommittee::summary_confirmation(&machine_id),
            super::MachineConfirmStatus::NoConsensus(super::Summary {
                unruly: vec![committee1],
                invalid_support: vec![committee2, committee3],
                ..Default::default()
            })
        );
    })
}

// case 11: 3提交Hash，2提交原始值，且都是支持，且两个相等
#[test]
fn test_summary_confirmation11() {
    new_test_with_init_params_ext().execute_with(|| {
        let machine_id = "484f457327950359de97c4b4c193bb3c8ddbe1dce56f038b3ac2b90e40995241".as_bytes().to_vec();

        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One).into();
        let committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Two).into();
        let committee3: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();

        run_to_block(10);

        // 构建 machine_committee
        <super::MachineCommittee<TestRuntime>>::insert(
            &machine_id,
            super::OCMachineCommitteeList {
                book_time: 9,
                booked_committee: vec![committee2, committee3, committee1],
                hashed_committee: vec![committee2, committee3],
                confirm_start_time: 5432,
                confirmed_committee: vec![committee2, committee3],
                onlined_committee: vec![],
                status: super::OCVerifyStatus::Summarizing,
            },
        );

        let machine_info_hash: [u8; 16] = hex::decode("d80b116fd318f19fd89da792aba5e875").unwrap().try_into().unwrap();

        let committee_ops = super::OCCommitteeOps {
            staked_dbc: 1000 * ONE_DBC,
            verify_time: vec![],
            confirm_hash: machine_info_hash.clone(),
            hash_time: 16887,
            confirm_time: 16891,
            machine_status: super::OCMachineStatus::Confirmed,
            machine_info: super::CommitteeUploadInfo {
                machine_id: "484f457327950359de97c4b4c193bb3c8ddbe1dce56f038b3ac2b90e40995241".as_bytes().to_vec(),
                gpu_type: "GeForceRTX3060".as_bytes().to_vec(),
                gpu_num: 4,
                cuda_core: 3584,
                gpu_mem: 12,
                calc_point: 41718,
                sys_disk: 256,
                data_disk: 1800,
                cpu_type: "Intel(R) Xeon(R) Platinum Intel 8259L CPU".as_bytes().to_vec(),
                cpu_core_num: 96,
                cpu_rate: 2400,
                mem_num: 192,
                rand_str: "".as_bytes().to_vec(),
                is_support: true,
            },
        };

        let committee2_ops = super::OCCommitteeOps {
            verify_time: vec![1142, 2582, 4022],
            confirm_hash: machine_info_hash.clone(),
            ..committee_ops.clone()
        };

        let committee3_ops = super::OCCommitteeOps {
            verify_time: vec![662, 2102, 3542],
            confirm_hash: machine_info_hash.clone(),
            ..committee_ops.clone()
        };

        // 构建committee_ops
        <CommitteeOps<TestRuntime>>::insert(&committee2, &machine_id, committee2_ops);
        <CommitteeOps<TestRuntime>>::insert(&committee3, &machine_id, committee3_ops);

        assert_eq!(
            OnlineCommittee::summary_confirmation(&machine_id),
            super::MachineConfirmStatus::Confirmed(super::Summary {
                unruly: vec![committee1],
                valid_support: vec![committee2, committee3],
                info: Some(committee_ops.machine_info.clone()),
                ..Default::default()
            })
        );
    })
}
