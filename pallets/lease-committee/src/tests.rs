#![allow(dead_code)]

use crate::{mock::*, LCCommitteeMachineList, LCMachineCommitteeList, LCVerifyStatus};
use committee::CommitteeList;
use frame_support::assert_ok;
use online_profile::{
    CommitteeUploadInfo, EraStashPoints, LiveMachine, MachineGradeStatus, MachineStatus, StakerCustomizeInfo,
    UserReonlineStakeInfo,
};
use sp_runtime::Perbill;
use std::{collections::BTreeMap, convert::TryInto};

#[test]
fn machine_online_works() {
    new_test_with_init_params_ext().execute_with(|| {
        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One).into();

        let controller: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();
        let stash: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();
        // Bob pubkey
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();
        let msg = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48\
                   5CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL";
        let sig = "3abb2adb1bad83b87d61be8e55c31cec4b3fb2ecc5ee7254c8df88b1ec92e025\
                   4f4a9b010e2d8a5cce9d262e9193b76be87b46f6bef4219517cf939520bfff84";

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
            init_stake_amount: 100000 * ONE_DBC,
            current_stake_amount: 100000 * ONE_DBC,
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

        // 添加了信息之后，将会在LeaseCommittee中被派单
        // add_machine_info
        // - Writes: MachinesInfo, LiveMachines, committee::CommitteeStake
        assert_eq!(&OnlineProfile::machines_info(&machine_id), &machine_info);
        assert_eq!(
            OnlineProfile::live_machines(),
            LiveMachine { confirmed_machine: vec!(machine_id.clone()), ..Default::default() }
        );

        // 增加一个委员会
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee1));
        let one_box_pubkey: [u8; 32] = hex::decode("9dccbab2d61405084eac440f877a6479bc827373b2e414e81a6170ebe5aadd12")
            .unwrap()
            .try_into()
            .unwrap();
        assert_ok!(Committee::committee_set_box_pubkey(Origin::signed(committee1), one_box_pubkey.clone()));

        // FIXME: bugs: check free_balance
        // assert_eq!(Balances::free_balance(committee1), INIT_BALANCE - 20000 * ONE_DBC);

        // 委员会处于正常状态(排序后的列表)
        assert_eq!(Committee::committee(), CommitteeList { normal: vec![committee1], ..Default::default() });
        // 获取可派单的委员会正常
        assert_ok!(LeaseCommittee::lucky_committee().ok_or(()));

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
            LeaseCommittee::machine_committee(machine_id.clone()),
            LCMachineCommitteeList {
                book_time: 4,
                confirm_start_time: 4324,
                booked_committee: vec![committee1],
                ..Default::default()
            }
        );
        assert_eq!(
            LeaseCommittee::committee_machine(&committee1),
            crate::LCCommitteeMachineList { booked_machine: vec![machine_id.clone()], ..Default::default() }
        );
        assert_eq!(
            LeaseCommittee::committee_ops(&committee1, &machine_id),
            crate::LCCommitteeOps {
                staked_dbc: 1000 * ONE_DBC,
                verify_time: vec![4, 484, 964, 1444, 1924, 2404, 2884, 3364, 3844],
                ..Default::default()
            }
        );

        // 委员会提交机器Hash
        let machine_info_hash: [u8; 16] = hex::decode("d80b116fd318f19fd89da792aba5e875").unwrap().try_into().unwrap();
        assert_ok!(LeaseCommittee::submit_confirm_hash(
            Origin::signed(committee1),
            machine_id.clone(),
            machine_info_hash
        ));

        let mut committee_upload_info = CommitteeUploadInfo {
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

            rand_str: "abcdefg".as_bytes().to_vec(),
            is_support: true,
        };

        // 委员会提交原始信息
        assert_ok!(LeaseCommittee::submit_confirm_raw(Origin::signed(committee1), committee_upload_info.clone()));

        // submit_confirm_raw:
        // - Writes: MachineSubmitedHash, MachineCommittee, CommitteeMachine, CommitteeOps
        assert_eq!(LeaseCommittee::machine_submited_hash(&machine_id), vec![machine_info_hash.clone()]);
        assert_eq!(
            LeaseCommittee::machine_committee(&machine_id),
            crate::LCMachineCommitteeList {
                book_time: 4,
                confirm_start_time: 4324,
                booked_committee: vec![committee1],
                hashed_committee: vec![committee1],
                confirmed_committee: vec![committee1],
                status: crate::LCVerifyStatus::Summarizing,
                ..Default::default()
            }
        );
        assert_eq!(
            LeaseCommittee::committee_machine(&committee1),
            crate::LCCommitteeMachineList { confirmed_machine: vec![machine_id.clone()], ..Default::default() }
        );
        committee_upload_info.rand_str = vec![];
        assert_eq!(
            LeaseCommittee::committee_ops(&committee1, machine_id.clone()),
            crate::LCCommitteeOps {
                staked_dbc: 1000 * ONE_DBC,
                verify_time: vec![4, 484, 964, 1444, 1924, 2404, 2884, 3364, 3844],
                confirm_hash: machine_info_hash,
                hash_time: 6,
                confirm_time: 6,
                machine_status: crate::LCMachineStatus::Confirmed,
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
            init_stake_amount: 400000 * ONE_DBC,
            current_stake_amount: 400000 * ONE_DBC,
            reward_deadline: 365 * 2,
            reward_committee: vec![committee1],
            machine_info_detail: online_profile::MachineInfoDetail {
                committee_upload_info,
                staker_customize_info: machine_info.machine_info_detail.staker_customize_info,
            },
            ..machine_info
        };

        let sys_info = online_profile::SysInfoDetail {
            total_gpu_num: 4,
            total_staker: 1,
            total_calc_points: 6828,
            total_stake: 400000 * ONE_DBC,
            ..Default::default()
        };

        assert_eq!(&OnlineProfile::machines_info(&machine_id), &machine_info);
        assert_eq!(
            OnlineProfile::pos_gpu_info(
                online_profile::Longitude::East(1157894),
                online_profile::Latitude::North(235678)
            ),
            online_profile::PosInfo { online_gpu: 4, online_gpu_calc_points: 6825, ..Default::default() }
        );
        assert_eq!(OnlineProfile::server_room_machines(server_room[0]), Some(vec![machine_id.clone()]));
        assert_eq!(&OnlineProfile::sys_info(), &sys_info);

        let mut staker_statistic = BTreeMap::new();
        staker_statistic.insert(
            stash,
            online_profile::StashMachineStatistics {
                online_gpu_num: 4,
                inflation: Perbill::from_rational_approximation(4u32, 10000),
                machine_total_calc_point: 6825,
                rent_extra_grade: 0,
            },
        );
        assert_eq!(OnlineProfile::eras_stash_points(0), Some(EraStashPoints { ..Default::default() }));
        assert_eq!(
            OnlineProfile::eras_stash_points(1),
            Some(EraStashPoints { total: 6828, staker_statistic: staker_statistic.clone() })
        );

        let mut era_machine_points = BTreeMap::new();
        assert_eq!(OnlineProfile::eras_machine_points(0), Some(BTreeMap::new()));
        era_machine_points.insert(
            machine_id.clone(),
            MachineGradeStatus { basic_grade: 6825, is_rented: false, reward_account: vec![committee1] },
        );
        assert_eq!(OnlineProfile::eras_machine_points(1), Some(era_machine_points));

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
            total_calc_points: 6828,
            total_gpu_num: 4,
            ..stash_machine_info
        };

        assert_eq!(&OnlineProfile::stash_machines(stash), &stash_machine_info);

        committee_stake_info.can_claim_reward = 2750 * ONE_DBC; // 1100000 * 0.25 * 0.01
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
            total_calc_points: 6828,
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
        committee_stake_info.can_claim_reward = 2750 * ONE_DBC * 2 + 54 * ONE_DBC + 9999945 * ONE_DBC / 10000000; // 1100000 * 0.25 * 0.01; 1100000 * 0.75 / 150 * 0.01
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
        // FIXME: 检查质押
        assert_eq!(Balances::free_balance(committee1), INIT_BALANCE - 20000 * ONE_DBC + 5554999994500000000);

        // NOTE: 测试 控制账户重新上线机器
        assert_ok!(OnlineProfile::offline_machine_change_hardware_info(Origin::signed(controller), machine_id.clone()));

        // - Writes:
        // LiveMachines, MachineInfo, StashStake, UserReonlineStake, PosGPUInfo, StashMachine
        // CurrentEraStashPoints, NextEraStashPoints, CurrentEraMachinePoints, NextEraMachinePoints, SysInfo,
        assert_eq!(
            OnlineProfile::live_machines(),
            LiveMachine { bonding_machine: vec![machine_id.clone()], ..Default::default() }
        );
        machine_info.machine_status = MachineStatus::StakerReportOffline(8643, Box::new(MachineStatus::Online));
        assert_eq!(&OnlineProfile::machines_info(&machine_id), &machine_info);
        assert_eq!(OnlineProfile::stash_stake(&stash), 2000 * ONE_DBC + 400000 * ONE_DBC);
        assert_eq!(
            OnlineProfile::user_reonline_stake(&stash, &machine_id),
            online_profile::UserReonlineStakeInfo { stake_amount: 2000 * ONE_DBC, offline_time: 2880 * 3 + 3 }
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
            online_profile::SysInfoDetail { total_stake: 400000 * ONE_DBC, ..Default::default() }
        );
        stash_machine_info.online_machine = vec![];
        stash_machine_info.total_gpu_num = 0;
        stash_machine_info.total_calc_points = 0;
        assert_eq!(&OnlineProfile::stash_machines(&stash), &stash_machine_info);

        // 当前Era为3
        assert_eq!(OnlineProfile::current_era(), 3);
        assert_eq!(OnlineProfile::eras_stash_points(3), Some(EraStashPoints { ..Default::default() }));
        assert_eq!(OnlineProfile::eras_stash_points(4), Some(EraStashPoints { ..Default::default() }));
        assert_eq!(OnlineProfile::eras_machine_points(3), Some(BTreeMap::new()));
        assert_eq!(OnlineProfile::eras_machine_points(4), Some(BTreeMap::new()));

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
            LeaseCommittee::machine_committee(machine_id.clone()),
            LCMachineCommitteeList {
                book_time: 2880 * 3 + 3,
                confirm_start_time: 2880 * 3 + 3 + 4320,
                booked_committee: vec![committee1],
                ..Default::default()
            }
        );
        assert_eq!(
            LeaseCommittee::committee_machine(&committee1),
            crate::LCCommitteeMachineList {
                booked_machine: vec![machine_id.clone()],
                online_machine: vec![machine_id.clone()],
                ..Default::default()
            }
        );
        assert_eq!(
            LeaseCommittee::committee_ops(&committee1, &machine_id),
            crate::LCCommitteeOps {
                staked_dbc: 1000 * ONE_DBC,
                verify_time: vec![8643, 9123, 9603, 10083, 10563, 11043, 11523, 12003, 12483],
                ..Default::default()
            }
        );

        // 委员会提交机器Hash
        let machine_info_hash: [u8; 16] = hex::decode("142facd4738cdf47bae49edef5171ebf").unwrap().try_into().unwrap();
        assert_ok!(LeaseCommittee::submit_confirm_hash(
            Origin::signed(committee1),
            machine_id.clone(),
            machine_info_hash.clone()
        ));

        // submit_confirm_hash:
        // - Writes:
        // MachineSubmitedHash, MachineCommittee, CommitteeMachine, CommitteeOps

        // 委员会提交原始信息
        let mut committee_upload_info = CommitteeUploadInfo {
            machine_id: machine_id.clone(),
            gpu_type: "GeForceRTX2080Ti".as_bytes().to_vec(),
            gpu_num: 8,
            cuda_core: 4352,
            gpu_mem: 11,
            calc_point: 6825 * 8,
            sys_disk: 480,
            data_disk: 18,
            cpu_type: "Intel(R) Xeon(R) Silver 4110 CPU".as_bytes().to_vec(),
            cpu_core_num: 32,
            cpu_rate: 260,
            mem_num: 512000,

            rand_str: "abcdefg".as_bytes().to_vec(),
            is_support: true,
        };
        assert_ok!(&LeaseCommittee::submit_confirm_raw(Origin::signed(committee1), committee_upload_info.clone()));

        // submit_confirm_raw:
        // - Writes: MachineSubmitedHash, MachineCommittee, CommitteeMachine, CommitteeOps
        assert_eq!(LeaseCommittee::machine_submited_hash(&machine_id), vec![machine_info_hash.clone()]);
        assert_eq!(
            LeaseCommittee::machine_committee(&machine_id),
            crate::LCMachineCommitteeList {
                book_time: 8643,
                confirm_start_time: 12963,
                booked_committee: vec![committee1],
                hashed_committee: vec![committee1],
                confirmed_committee: vec![committee1],
                status: crate::LCVerifyStatus::Summarizing,
                ..Default::default()
            }
        );
        assert_eq!(
            LeaseCommittee::committee_machine(&committee1),
            crate::LCCommitteeMachineList {
                confirmed_machine: vec![machine_id.clone()],
                online_machine: vec![machine_id.clone()],
                ..Default::default()
            }
        );
        committee_upload_info.rand_str = vec![];
        assert_eq!(
            LeaseCommittee::committee_ops(&committee1, machine_id.clone()),
            crate::LCCommitteeOps {
                staked_dbc: 1000 * ONE_DBC,
                verify_time: vec![8643, 9123, 9603, 10083, 10563, 11043, 11523, 12003, 12483],
                confirm_hash: machine_info_hash,
                hash_time: 8644,
                confirm_time: 8644,
                machine_status: crate::LCMachineStatus::Confirmed,
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
            init_stake_amount: 800000 * ONE_DBC,
            current_stake_amount: 784000 * ONE_DBC, // 扣除2%的machine_stake质押
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
                total_calc_points: 54644, // 6825 * 8 * (1 + 8/10000) = 54600 + 43.68 = 54644
                total_stake: 800000 * ONE_DBC,
                ..Default::default()
            }
        );
        assert_eq!(
            OnlineProfile::user_reonline_stake(&stash, &machine_id),
            UserReonlineStakeInfo { ..Default::default() }
        );
        assert_eq!(OnlineProfile::stash_stake(&stash), 800000 * ONE_DBC);
        // 检查分数

        let mut staker_statistic = BTreeMap::new();
        staker_statistic.insert(
            stash,
            online_profile::StashMachineStatistics {
                online_gpu_num: 8,
                inflation: Perbill::from_rational_approximation(8u32, 10000),
                machine_total_calc_point: 54600,
                rent_extra_grade: 0,
            },
        );
        assert_eq!(
            OnlineProfile::eras_stash_points(4),
            Some(EraStashPoints { total: 54644, staker_statistic: staker_statistic.clone() })
        );

        let mut era_machine_points = BTreeMap::new();
        era_machine_points.insert(
            machine_id.clone(),
            MachineGradeStatus { basic_grade: 54600, is_rented: false, reward_account: vec![committee1] },
        );
        assert_eq!(OnlineProfile::eras_machine_points(4), Some(era_machine_points));
        assert_eq!(
            OnlineProfile::user_reonline_stake(&stash, &machine_id),
            online_profile::UserReonlineStakeInfo { ..Default::default() }
        );
        // 奖励2000DBC
        assert_eq!(
            Balances::free_balance(committee1),
            INIT_BALANCE - 20000 * ONE_DBC + 5554999994500000000 + 2000 * ONE_DBC
        );
    });
}

// 测试多个委员会分派，全部提交hash，原始信息后,奖励正常，惩罚正常
#[test]
fn all_committee_submit_works() {
    new_test_with_online_machine_distribution().execute_with(|| {
        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Alice).into();
        let committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Charlie).into();
        let _committee3: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Dave).into();
        let committee4: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();

        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();

        // 查询机器中对应的委员会列表
        assert_eq!(
            LeaseCommittee::machine_committee(machine_id.clone()),
            LCMachineCommitteeList {
                book_time: 6,
                booked_committee: vec![committee2, committee1, committee4],
                confirm_start_time: 6 + 4320,
                status: LCVerifyStatus::SubmittingHash,
                ..Default::default()
            }
        );

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
        let rand_str4 = "abcdefg3".as_bytes().to_vec();

        // 委员会提交机器Hash
        let machine_info_hash1 = hex::decode("f813d5478a1c6cfb04a203a0643ad67e").unwrap().try_into().unwrap();
        let machine_info_hash2 = hex::decode("8beab87415978daf436f31a292f9bdbb").unwrap().try_into().unwrap();
        let machine_info_hash4 = hex::decode("b76f264dbfeba1c25fb0518ed156ab40").unwrap().try_into().unwrap();

        assert_ok!(LeaseCommittee::submit_confirm_hash(
            Origin::signed(committee1),
            machine_id.clone(),
            machine_info_hash1,
        ));

        assert_ok!(LeaseCommittee::submit_confirm_hash(
            Origin::signed(committee2),
            machine_id.clone(),
            machine_info_hash2,
        ));
        assert_ok!(LeaseCommittee::submit_confirm_hash(
            Origin::signed(committee4),
            machine_id.clone(),
            machine_info_hash4,
        ));

        // 委员会提交原始信息
        assert_ok!(LeaseCommittee::submit_confirm_raw(
            Origin::signed(committee1),
            CommitteeUploadInfo { rand_str: rand_str1, ..machine_base_info.clone() }
        ));
        assert_ok!(LeaseCommittee::submit_confirm_raw(
            Origin::signed(committee2),
            CommitteeUploadInfo { rand_str: rand_str2, ..machine_base_info.clone() }
        ));

        assert_ok!(LeaseCommittee::submit_confirm_raw(
            Origin::signed(committee4),
            CommitteeUploadInfo { rand_str: rand_str4, ..machine_base_info.clone() }
        ));

        // 检查结果

        // TODO: 检查奖励分配
    })
}

// TODO: 三个委员会两个正常工作，一个不提交Hash值，检查惩罚机制
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

        assert_ok!(LeaseCommittee::submit_confirm_hash(
            Origin::signed(committee1),
            machine_id.clone(),
            machine_info_hash1,
        ));
        assert_ok!(LeaseCommittee::submit_confirm_hash(
            Origin::signed(committee2),
            machine_id.clone(),
            machine_info_hash2,
        ));

        let machine_committee = crate::LCMachineCommitteeList {
            book_time: 6,
            booked_committee: vec![committee2, committee1, committee4],
            hashed_committee: vec![committee2, committee1],
            confirm_start_time: 4326,
            confirmed_committee: vec![],
            onlined_committee: vec![],
            status: crate::LCVerifyStatus::default(),
        };

        assert_eq!(machine_committee, LeaseCommittee::machine_committee(&machine_id),);

        // 等到36个小时之后，提交确认信息
        run_to_block(4326); // 6 + 36 * 120 = 4326

        // 委员会提交原始信息
        assert_ok!(LeaseCommittee::submit_confirm_raw(
            Origin::signed(committee1),
            CommitteeUploadInfo { rand_str: rand_str1, ..machine_base_info.clone() }
        ));
        assert_ok!(LeaseCommittee::submit_confirm_raw(
            Origin::signed(committee2),
            CommitteeUploadInfo { rand_str: rand_str2, ..machine_base_info.clone() }
        ));

        run_to_block(4327);

        assert_eq!(
            OnlineProfile::live_machines(),
            online_profile::LiveMachine { online_machine: vec![machine_id.clone()], ..Default::default() }
        );

        // Committee 记录到了惩罚信息
        assert_eq!(
            Committee::pending_slash(0),
            committee::CMPendingSlashInfo {
                slash_who: committee4,
                slash_time: 4327,
                unlock_amount: 1000 * ONE_DBC,
                slash_amount: 1000 * ONE_DBC,
                slash_exec_time: 4327 + 2880 * 2, // 2day
                reward_to: vec![]
            }
        );
        // 惩罚
        run_to_block(4327 + 2880 * 2 + 1);
    })
}

// TODO: 三个委员会两个正常工作，一个提交Hash之后，没有提交原始值，检查惩罚机制
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
    // new_test_with_online_machine_distribution().execute_with(|| {
    new_test_with_init_params_ext().execute_with(|| {
        // 委员会需要提交的信息
        let mut machine_3080 = online_profile::CommitteeUploadInfo {
            gpu_type: "GeForceRTX3080".as_bytes().to_vec(),
            gpu_num: 4,
            cuda_core: 8704,
            gpu_mem: 10,
            calc_point: 59890,
            sys_disk: 500,
            data_disk: 3905,
            cpu_type: "Intel(R) Xeon(R) Silver 4214R".as_bytes().to_vec(),
            cpu_core_num: 64,
            cpu_rate: 2400,
            mem_num: 440,

            is_support: true,

            ..Default::default()
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

        let machine_info_hash1: [u8; 16] = hex::decode("6b561dfad171810dfb69924dd68733ec").unwrap().try_into().unwrap();
        let machine_info_hash2: [u8; 16] = hex::decode("5b4499c4b6e9f080673f9573410a103a").unwrap().try_into().unwrap();
        let machine_info_hash3: [u8; 16] = hex::decode("3ac5b3416d1743b58a4c9af58c7002d7").unwrap().try_into().unwrap();

        let controller: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Alice).into();
        let stash: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Bob).into();

        // Machine account Info:
        // ❯ subkey generate --scheme sr25519
        //   Secret seed:       0x16f2e4b3ad50aab4f5c7ab56d793738a893080d578976040a5be284da12437b6
        //   Public key (hex):  0xdc763e931919cceee0c35392d124c753fc4e4ab6e494bc67722fdd31989d660f

        let machine_id = "dc763e931919cceee0c35392d124c753fc4e4ab6e494bc67722fdd31989d660f".as_bytes().to_vec();
        let msg = "dc763e931919cceee0c35392d124c753fc4e4ab6e494bc67722fdd31989d660f\
                   5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty";
        let sig = "d43429622b5754557bc0ccc39e29d717d6d103633777082bd0c5deffbade94\
           224804c95ca06bcd5f99f5c9302c14ed26cef32d474163ec9a201afd0fcee0d189";

        machine_3080.machine_id = machine_id.clone();

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
            init_stake_amount: 100000 * ONE_DBC,
            current_stake_amount: 100000 * ONE_DBC,
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

        machine_info.machine_status = online_profile::MachineStatus::CommitteeVerifying;

        // 正常Hash: 0x6b561dfad171810dfb69924dd68733ec
        // cpu_core_num: 48: 0x5b4499c4b6e9f080673f9573410a103a
        // cpu_core_num: 96: 0x3ac5b3416d1743b58a4c9af58c7002d7

        // 三个委员会分别提交机器Hash
        assert_ok!(LeaseCommittee::submit_confirm_hash(
            Origin::signed(committee1),
            machine_id.clone(),
            machine_info_hash1
        ));
        assert_ok!(LeaseCommittee::submit_confirm_hash(
            Origin::signed(committee2),
            machine_id.clone(),
            machine_info_hash2
        ));
        assert_ok!(LeaseCommittee::submit_confirm_hash(
            Origin::signed(committee3),
            machine_id.clone(),
            machine_info_hash3
        ));

        machine_3080.rand_str = "abcdefg1".as_bytes().to_vec();
        // 委员会提交原始信息
        assert_ok!(LeaseCommittee::submit_confirm_raw(Origin::signed(committee1), machine_3080.clone()));
        machine_3080.cpu_core_num = 48;
        assert_ok!(LeaseCommittee::submit_confirm_raw(Origin::signed(committee2), machine_3080.clone()));
        machine_3080.cpu_core_num = 96;
        assert_ok!(LeaseCommittee::submit_confirm_raw(Origin::signed(committee3), machine_3080.clone()));

        assert_eq!(
            LeaseCommittee::machine_committee(&machine_id),
            super::LCMachineCommitteeList { ..Default::default() }
        );

        run_to_block(17);

        // 机器直接删掉信息即可
        assert_eq!(LeaseCommittee::committee_machine(&committee1), LCCommitteeMachineList { ..Default::default() });
        assert_eq!(LeaseCommittee::committee_machine(&committee2), LCCommitteeMachineList { ..Default::default() });
        assert_eq!(LeaseCommittee::committee_machine(&committee3), LCCommitteeMachineList { ..Default::default() });

        // 如果on_finalize先执行lease_committee 再z执行online_profile则没有内容，否则被重新分配了
        assert_eq!(
            LeaseCommittee::machine_committee(&machine_id),
            super::LCMachineCommitteeList { ..Default::default() }
        );

        let machine_submit_hash: Vec<[u8; 16]> = vec![];
        assert_eq!(LeaseCommittee::machine_submited_hash(&machine_id), machine_submit_hash);
        assert_eq!(
            LeaseCommittee::committee_ops(&committee1, &machine_id),
            super::LCCommitteeOps { ..Default::default() }
        );

        // TODO: 测试清理信息
    })
}
