#![allow(dead_code)]

use crate::{mock::*, LCMachineCommitteeList, LCVerifyStatus};
use committee::CommitteeList;
use frame_support::assert_ok;
use online_profile::{
    CommitteeUploadInfo, EraStashPoints, LiveMachine, MachineGradeStatus, MachineInfo, MachineStatus,
    StakerCustomizeInfo,
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

        let account: sp_core::sr25519::Public = sr25519::Public::from_raw(
            hex::decode("2f8c6129d816cf51c374bc7f08c3e63ed156cf78aefb4a6550d97b87997977ee")
                .unwrap()
                .try_into()
                .unwrap(),
        );

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

        let mut stash_machine_info =
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

        let mut sys_info = online_profile::SysInfoDetail {
            total_gpu_num: 4,
            total_staker: 1,
            total_calc_points: 6828,
            total_stake: 400000 * ONE_DBC,
            ..Default::default()
        };

        assert_eq!(&OnlineProfile::machines_info(&machine_id), &machine_info);
        // TODO:
        assert_eq!(
            OnlineProfile::pos_gpu_info(online_profile::Longitude::East(1), online_profile::Latitude::North(1)),
            online_profile::PosInfo { ..Default::default() }
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
        assert_eq!(OnlineProfile::eras_stash_points(1), Some(EraStashPoints { total: 6828, staker_statistic }));

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

        let mut stash_machine_info = online_profile::StashMachine {
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
        assert_eq!(Balances::free_balance(stash), INIT_BALANCE + 549944999455500000000);

        // 委员会领取奖励
        // - Writes:
        // CommitteeStake, Committee Balance
        assert_ok!(Committee::claim_reward(Origin::signed(committee1)));
        committee_stake_info.claimed_reward = committee_stake_info.can_claim_reward;
        committee_stake_info.can_claim_reward = 0;
        assert_eq!(&Committee::committee_stake(&committee1), &committee_stake_info);
        assert_eq!(Balances::free_balance(committee1), INIT_BALANCE + 5554999994500000000);

        // NOTE: 测试 控制账户重新上线机器
        assert_ok!(OnlineProfile::offline_machine_change_hardware_info(Origin::signed(controller), machine_id.clone()));

        // - Writes:
        // LiveMachines, MachineInfo, UserReonlineStake, PosGPUInfo,
        // CurrentEraStashPoints, NextEraStashPoints, CurrentEraMachinePoints, NextEraMachinePoints, SysInfo, StashMachine
        assert_eq!(
            OnlineProfile::live_machines(),
            LiveMachine { bonding_machine: vec![machine_id.clone()], ..Default::default() }
        );

        machine_info.machine_status = MachineStatus::StakerReportOffline(8643, Box::new(MachineStatus::Online));
        assert_eq!(&OnlineProfile::machines_info(&machine_id), &machine_info);
        let user_reonline_stake = (1, machine_info.machine_status.clone());
        // assert_eq!(OnlineProfile::user_reonline_stake, user_reonline_stake); // FIXME: 添加判断
        // Skip POsGPUInfo

        assert_eq!(
            OnlineProfile::sys_info(),
            online_profile::SysInfoDetail { total_stake: 400000 * ONE_DBC, ..Default::default() }
        );
        stash_machine_info.online_machine = vec![];
        stash_machine_info.total_gpu_num = 0;
        stash_machine_info.total_calc_points = 0;
        assert_eq!(&OnlineProfile::stash_machines(&stash), &stash_machine_info);

        // 控制账户重新添加机器信息
        assert_ok!(OnlineProfile::add_machine_info(
            Origin::signed(controller),
            machine_id.clone(),
            StakerCustomizeInfo {
                server_room: server_room[0],
                upload_net: 10000,
                download_net: 10000,
                longitude: online_profile::Longitude::East(1157894),
                latitude: online_profile::Latitude::North(235678),
                telecom_operators: vec!["China Unicom".into()],
            }
        ));

        run_to_block(2880 * 3 + 3);

        // 委员会重新上链
        // 查询机器中有订阅的委员会
        assert_eq!(
            LeaseCommittee::machine_committee(machine_id.clone()),
            LCMachineCommitteeList {
                book_time: 8643,
                confirm_start_time: 8643 + 4320,
                booked_committee: vec![committee1],
                ..Default::default()
            }
        );

        // 委员会提交机器Hash
        let machine_info_hash = "142facd4738cdf47bae49edef5171ebf";
        assert_ok!(LeaseCommittee::submit_confirm_hash(
            Origin::signed(committee1),
            machine_id.clone(),
            hex::decode(machine_info_hash).unwrap().try_into().unwrap()
        ));

        // 委员会提交原始信息
        assert_ok!(LeaseCommittee::submit_confirm_raw(
            Origin::signed(committee1),
            CommitteeUploadInfo {
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
            }
        ));

        run_to_block(2880 * 3 + 4);

        assert_eq!(
            OnlineProfile::live_machines(),
            LiveMachine { online_machine: vec!(machine_id.clone()), ..Default::default() }
        );
        assert_eq!(
            OnlineProfile::sys_info(),
            online_profile::SysInfoDetail {
                total_gpu_num: 8,
                total_staker: 1,
                total_calc_points: 54644,
                total_stake: 800000 * ONE_DBC,
                ..Default::default()
            }
        );
    });
}

// 测试多个委员会分派，全部提交hash，原始信息后,奖励正常，惩罚正常
#[test]
fn all_committee_submit_works() {
    new_test_with_online_machine_distribution().execute_with(|| {
        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Alice).into();
        let committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Charlie).into();
        let committee3: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Dave).into();
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
        let committee3: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Dave).into();
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
            committee::PendingSlashInfo {
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
fn committee_not_wubmit_raw_slash_works() {
    new_test_with_online_machine_distribution().execute_with(|| {
        let _committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Alice).into();
        let committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Charlie).into();
        let committee3: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Dave).into();
        let committee4: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();

        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();
    })
}

#[test]
fn fulfill_should_work() {
    new_test_with_online_machine_distribution().execute_with(|| {})
}
