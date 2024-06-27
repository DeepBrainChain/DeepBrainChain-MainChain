use super::super::{mock::*, OCCommitteeMachineList, OCMachineCommitteeList, *};
use committee::CommitteeList;
use dbc_support::{
    live_machine::LiveMachine,
    machine_info::MachineInfo,
    machine_type::{
        CommitteeUploadInfo, Latitude, Longitude, MachineInfoDetail, MachineStatus,
        StakerCustomizeInfo,
    },
    verify_online::StashMachine,
};
use frame_support::assert_ok;
use online_profile::{EraStashPoints, MachineGradeStatus, UserMutHardwareStakeInfo};
use sp_runtime::Perbill;
use std::{collections::BTreeMap, convert::TryInto};

#[test]
fn machine_online_works() {
    new_test_with_init_params_ext().execute_with(|| {
        let committee1 = sr25519::Public::from(Sr25519Keyring::One);
        let committee2 = sr25519::Public::from(Sr25519Keyring::Two);
        let committee3 = sr25519::Public::from(Sr25519Keyring::Dave);

        let controller = sr25519::Public::from(Sr25519Keyring::Eve);
        let stash = sr25519::Public::from(Sr25519Keyring::Ferdie);
        // Bob pubkey
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();
        let msg = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48\
                   5CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL";
        let sig = "b4084f70730b183127e9db78c6d8dcf79039f23466cd1ee8b536c40c3027a83d\
                   ab040be4ed2db57b67eaac406817a69ce72a13f8ac11ba460e15d318b1504481";

        {
            // 查询初始状态
            assert_eq!(Balances::free_balance(committee1), INIT_BALANCE);
            assert_eq!(DBCPriceOCW::avg_price(), Some(12_000u64));
        }

        // stash 账户设置控制账户
        assert_ok!(OnlineProfile::set_controller(RuntimeOrigin::signed(stash), controller));

        // controller 生成server_room
        assert_ok!(OnlineProfile::gen_server_room(RuntimeOrigin::signed(controller)));
        assert_ok!(OnlineProfile::gen_server_room(RuntimeOrigin::signed(controller)));
        let server_room = OnlineProfile::stash_server_rooms(&stash);

        assert_ok!(OnlineProfile::bond_machine(
            RuntimeOrigin::signed(controller),
            machine_id.clone(),
            msg.as_bytes().to_vec(),
            hex::decode(sig).unwrap()
        ));

        let mut machine_info = MachineInfo {
            controller,
            machine_stash: stash,
            bonding_height: 3,
            stake_amount: 1000 * ONE_DBC,
            init_stake_per_gpu: 1000 * ONE_DBC,
            machine_status: MachineStatus::AddingCustomizeInfo,
            renters: vec![],
            last_machine_restake: 0,
            online_height: 0,
            last_online_height: 0,
            total_rented_duration: 0,
            total_rented_times: 0,
            total_rent_fee: 0,
            total_burn_fee: 0,
            machine_info_detail: Default::default(),
            reward_committee: vec![],
            reward_deadline: 0,
        };

        // bond_machine:
        // - Writes: ControllerMachines, StashMachines, LiveMachines, MachinesInfo, SysInfo,
        //   StashStake

        let stash_machine_info =
            StashMachine { total_machine: vec![machine_id.clone()], ..Default::default() };
        assert_eq!(OnlineProfile::controller_machines(&controller), vec!(machine_id.clone()));
        assert_eq!(&OnlineProfile::stash_machines(&stash), &stash_machine_info);
        assert_eq!(
            OnlineProfile::live_machines(),
            LiveMachine { bonding_machine: vec!(machine_id.clone()), ..Default::default() }
        );
        assert_eq!(OnlineProfile::machines_info(&machine_id), Some(machine_info.clone()));
        assert_eq!(
            OnlineProfile::sys_info(),
            online_profile::SysInfoDetail {
                total_staker: 0,
                total_stake: 1000 * ONE_DBC,
                ..Default::default()
            }
        );
        assert_eq!(OnlineProfile::stash_stake(&stash), 1000 * ONE_DBC);
        // 查询Controller支付30 DBC手续费: 绑定机器/添加机房信息各花费10DBC
        assert_eq!(Balances::free_balance(controller), INIT_BALANCE - 30 * ONE_DBC);

        let customize_info = StakerCustomizeInfo {
            server_room: server_room[0],
            upload_net: 100,
            download_net: 100,
            longitude: Longitude::East(1157894),
            latitude: Latitude::North(235678),
            telecom_operators: vec!["China Unicom".into()],
        };
        // 控制账户添加机器信息
        assert_ok!(OnlineProfile::add_machine_info(
            RuntimeOrigin::signed(controller),
            machine_id.clone(),
            customize_info.clone()
        ));

        machine_info.machine_info_detail.staker_customize_info = customize_info;
        machine_info.machine_status = MachineStatus::DistributingOrder;

        run_to_block(3);

        {
            // 添加了信息之后，将会在OnlineCommittee中被派单
            // add_machine_info
            // - Writes: MachinesInfo, LiveMachines, committee::CommitteeStake
            assert_eq!(OnlineProfile::machines_info(&machine_id), Some(machine_info.clone()));
            assert_eq!(
                OnlineProfile::live_machines(),
                LiveMachine { confirmed_machine: vec!(machine_id.clone()), ..Default::default() }
            );
        }

        // 增加一个委员会
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee1));
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee2));
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee3));

        let one_box_pubkey: [u8; 32] =
            hex::decode("9dccbab2d61405084eac440f877a6479bc827373b2e414e81a6170ebe5aadd12")
                .unwrap()
                .try_into()
                .unwrap();
        assert_ok!(Committee::committee_set_box_pubkey(
            RuntimeOrigin::signed(committee1),
            one_box_pubkey
        ));
        assert_ok!(Committee::committee_set_box_pubkey(
            RuntimeOrigin::signed(committee2),
            one_box_pubkey
        ));
        assert_ok!(Committee::committee_set_box_pubkey(
            RuntimeOrigin::signed(committee3),
            one_box_pubkey
        ));

        // 委员会处于正常状态(排序后的列表)
        assert_eq!(
            Committee::committee(),
            CommitteeList {
                normal: vec![committee2, committee3, committee1],
                ..Default::default()
            }
        );
        // 获取可派单的委员会正常
        assert_ok!(OnlineCommittee::get_work_index().ok_or(()));

        run_to_block(5);

        let mut committee_stake_info = committee::CommitteeStakeInfo {
            box_pubkey: one_box_pubkey,
            staked_amount: 20000 * ONE_DBC,
            used_stake: 1000 * ONE_DBC,
            ..Default::default()
        };

        machine_info.machine_status = MachineStatus::CommitteeVerifying;

        // Do distribute_machines:
        // - Writes: op::MachinesInfo, op::LiveMachines, committee::CommitteeStake,
        // lc::MachineCommittee, lc::CommitteeMachine, lc::CommitteeOps
        assert_eq!(&Committee::committee_stake(&committee1), &committee_stake_info);
        assert_eq!(
            OnlineProfile::live_machines(),
            LiveMachine { booked_machine: vec![machine_id.clone()], ..Default::default() }
        );
        assert_eq!(OnlineProfile::machines_info(&machine_id), Some(machine_info.clone()));

        assert_eq!(
            OnlineCommittee::machine_committee(machine_id.clone()),
            OCMachineCommitteeList {
                book_time: 4,
                confirm_start_time: 4324,
                booked_committee: vec![committee2, committee3, committee1],
                hashed_committee: vec![],
                confirmed_committee: vec![],
                onlined_committee: vec![],
                status: Default::default()
            }
        );
        assert_eq!(
            OnlineCommittee::committee_machine(&committee1),
            crate::OCCommitteeMachineList {
                booked_machine: vec![machine_id.clone()],
                ..Default::default()
            }
        );
        assert_eq!(
            OnlineCommittee::committee_ops(&committee3, &machine_id),
            crate::OCCommitteeOps {
                staked_dbc: 1000 * ONE_DBC,
                verify_time: vec![4, 1444, 2884],
                ..Default::default()
            }
        );

        // 委员会提交机器Hash
        let machine_info_hash1: [u8; 16] =
            hex::decode("fd8885a22a9d9784adaa36effcd77522").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            RuntimeOrigin::signed(committee1),
            machine_id.clone(),
            machine_info_hash1
        ));
        let machine_info_hash2: [u8; 16] =
            hex::decode("c016090e0943c17f5d4999dc6eb52683").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            RuntimeOrigin::signed(committee2),
            machine_id.clone(),
            machine_info_hash2
        ));
        let machine_info_hash3: [u8; 16] =
            hex::decode("4a6b2df1e1a77b9bcdab5e31dc7950d2").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            RuntimeOrigin::signed(committee3),
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
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            RuntimeOrigin::signed(committee1),
            committee_upload_info.clone()
        ));
        committee_upload_info.rand_str = "abcdefg2".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            RuntimeOrigin::signed(committee2),
            committee_upload_info.clone()
        ));
        committee_upload_info.rand_str = "abcdefg3".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            RuntimeOrigin::signed(committee3),
            committee_upload_info.clone()
        ));

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
                onlined_committee: Default::default()
            }
        );
        assert_eq!(
            OnlineCommittee::committee_machine(&committee1),
            crate::OCCommitteeMachineList {
                confirmed_machine: vec![machine_id.clone()],
                ..Default::default()
            }
        );
        committee_upload_info.rand_str = vec![];
        assert_eq!(
            OnlineCommittee::committee_ops(&committee1, machine_id.clone()),
            crate::OCCommitteeOps {
                staked_dbc: 1000 * ONE_DBC,
                verify_time: vec![484, 1924, 3364],
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
        // LiveMachines, MachinesInfo, PosGPU, ServerRoomGPU, SysInfo, StashMachines,
        // ErasStashPoints, ErasMachinePoints,

        // 检查机器状态
        assert_eq!(
            OnlineProfile::live_machines(),
            LiveMachine { online_machine: vec!(machine_id.clone()), ..Default::default() }
        );

        let mut machine_info = MachineInfo {
            machine_status: MachineStatus::Online,
            last_machine_restake: 6,
            online_height: 6,
            last_online_height: 6,
            stake_amount: 4000 * ONE_DBC,
            reward_deadline: 365 * 2 + 1,
            reward_committee: vec![committee2, committee3, committee1],
            machine_info_detail: MachineInfoDetail {
                committee_upload_info,
                staker_customize_info: machine_info.machine_info_detail.staker_customize_info,
            },
            ..machine_info
        };

        let sys_info = online_profile::SysInfoDetail {
            total_gpu_num: 4,
            total_staker: 1,
            total_calc_points: 59914, // 59890 + 59890 * 4/10000) = +24
            total_stake: 4000 * ONE_DBC,
            ..Default::default()
        };

        assert_eq!(OnlineProfile::machines_info(&machine_id), Some(machine_info.clone()));
        assert_eq!(
            OnlineProfile::pos_gpu_info(Longitude::East(1157894), Latitude::North(235678)),
            online_profile::PosInfo {
                online_gpu: 4,
                online_gpu_calc_points: 59890,
                ..Default::default()
            }
        );
        assert_eq!(&OnlineProfile::sys_info(), &sys_info);
        let mut staker_statistic = BTreeMap::new();
        staker_statistic.insert(
            stash,
            online_profile::StashMachineStatistics {
                online_gpu_num: 4,
                inflation: Perbill::from_rational(4u32, 10000),
                machine_total_calc_point: 59890,
                rent_extra_grade: 0,
            },
        );
        assert_eq!(OnlineProfile::eras_stash_points(0), EraStashPoints { ..Default::default() });
        assert_eq!(
            OnlineProfile::eras_stash_points(2),
            EraStashPoints { total: 59914, staker_statistic: staker_statistic.clone() }
        );

        let mut era_machine_points = BTreeMap::new();
        assert_eq!(OnlineProfile::eras_machine_points(0), BTreeMap::new());
        era_machine_points.insert(
            machine_id.clone(),
            MachineGradeStatus { basic_grade: 59890, is_rented: false },
        );
        assert_eq!(OnlineProfile::eras_machine_points(2), era_machine_points);

        // 过一个Era: 一天是2880个块
        run_to_block(2880 * 2 + 2);

        // do distribute_reward
        // - Writes:
        // ErasMachineReleasedReward, ErasMachineReward
        // ErasStashReleasedReward, ErasStashReward, StashMachines, committee reward

        assert_eq!(OnlineProfile::eras_machine_reward(1, &machine_id), 0);
        assert_eq!(OnlineProfile::eras_machine_reward(2, &machine_id), 1089000 * ONE_DBC); // 1100000 * 0.99
        assert_eq!(OnlineProfile::eras_machine_released_reward(1, &machine_id), 0);
        assert_eq!(OnlineProfile::eras_machine_released_reward(2, &machine_id), 272250 * ONE_DBC); // 1100000 * 0.99 * 0.25

        assert_eq!(OnlineProfile::eras_stash_reward(1, &stash), 0);
        assert_eq!(OnlineProfile::eras_stash_reward(2, &stash), 1089000 * ONE_DBC);
        assert_eq!(OnlineProfile::eras_stash_released_reward(1, &stash), 0);
        assert_eq!(OnlineProfile::eras_stash_released_reward(2, &stash), 272250 * ONE_DBC); // 1100000 * 0.99 * 0.25

        let stash_machine_info = StashMachine {
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

        assert_eq!(OnlineProfile::eras_machine_reward(1, &machine_id), 0);
        assert_eq!(OnlineProfile::eras_machine_reward(2, &machine_id), 1089000 * ONE_DBC); // 1100000 * 0.99
        assert_eq!(OnlineProfile::eras_machine_reward(3, &machine_id), 1089000 * ONE_DBC); // 1100000 * 0.99
        assert_eq!(OnlineProfile::eras_machine_released_reward(1, &machine_id), 0);
        assert_eq!(OnlineProfile::eras_machine_released_reward(2, &machine_id), 272250 * ONE_DBC); // 1100000 * 0.99 * 0.25

        // 释放剩余奖励的1/150: 1100000 * 0.75 / 150 * 0.99 = 5445;
        // 第一天奖励，在第二天线性释放，委员会获得的部分:
        let first_day_linear_release = 5445 * ONE_DBC;
        assert_eq!(
            OnlineProfile::eras_machine_released_reward(3, &machine_id),
            272250 * ONE_DBC + first_day_linear_release
        ); // 1100000 * 0.99 * 0.25 + 1100000 * 0.75 * 0.99 / 150

        let mut stash_machine_info = StashMachine {
            can_claim_reward: 272250 * 2 * ONE_DBC + first_day_linear_release, // 1100000 * 0.99 * 0.25
            online_machine: vec![machine_id.clone()],
            total_earned_reward: 1089000 * ONE_DBC * 2,
            total_calc_points: 59914,
            total_gpu_num: 4,
            ..stash_machine_info
        };

        assert_eq!(OnlineProfile::eras_stash_reward(1, &stash), 0);
        assert_eq!(OnlineProfile::eras_stash_reward(2, &stash), 1089000 * ONE_DBC);
        assert_eq!(OnlineProfile::eras_stash_reward(3, &stash), 1089000 * ONE_DBC);
        assert_eq!(OnlineProfile::eras_stash_released_reward(1, &stash), 0);
        assert_eq!(OnlineProfile::eras_stash_released_reward(2, &stash), 272250 * ONE_DBC); // 1100000 * 0.99 * 0.25
        assert_eq!(
            OnlineProfile::eras_stash_released_reward(3, &stash),
            272250 * ONE_DBC + first_day_linear_release
        ); // 1100000 * 0.99 * 0.25

        assert_eq!(&OnlineProfile::stash_machines(stash), &stash_machine_info);

        // 第二天释放的获得的第一天的将奖励： 1100000 * 0.75 * 6666666 / 10**9 * 0.01 = 54.9999945
        // 916 * ONE_DBC + 66666575 * 10_000_000 = 916.66666575; (1/150 -> 6666666 / 10**9), 1100000
        // * 0.75 * (6666666 / 10**9) * 0.01 * 333333333 / 10**9 = 18.33333148166667

        // 第二天释放的获得的第一天的将奖励： 1100000 * 0.25 / 100 * 2 + 1100000*0.75/150/100 =
        // 5555; Each committee two get: 5555 / 3 = 1851.66666666...

        // 1100000 * ONE_DBC * 0.25 / 3 * 2 + 55 / 3 * ONE_DBC;
        committee_stake_info.can_claim_reward =
            (916 * ONE_DBC + 66666575 * 10_000_000) * 2 + 18 * ONE_DBC + 333333315000000;
        committee_stake_info.used_stake = 0;
        assert_eq!(&Committee::committee_stake(&committee1), &committee_stake_info);

        stash_machine_info.total_earned_reward = 2178000 * ONE_DBC; // 1100000 * 0.99 * 2
        assert_eq!(&OnlineProfile::stash_machines(&stash), &stash_machine_info);
        // 矿工领取奖励
        // - Writes:
        // StashMachines, User Balances
        // 领取奖励前质押4000 领取奖励后部分奖励进入质押 质押金额为40w
        assert_ok!(OnlineProfile::claim_rewards(RuntimeOrigin::signed(controller)));
        // 领取奖励后，查询剩余奖励
        stash_machine_info.total_claimed_reward = stash_machine_info.can_claim_reward;
        stash_machine_info.can_claim_reward = 0;
        machine_info.stake_amount = 400000 * ONE_DBC;
        assert_eq!(&OnlineProfile::stash_machines(&stash), &stash_machine_info);

        // 领取奖励后，查询账户余额
        assert_eq!(
            Balances::free_balance(stash),
            INIT_BALANCE - 400000 * ONE_DBC + 549945 * ONE_DBC
        );

        // 委员会领取奖励
        // - Writes:
        // CommitteeStake, Committee Balance
        assert_ok!(Committee::claim_reward(RuntimeOrigin::signed(committee1)));
        committee_stake_info.claimed_reward = committee_stake_info.can_claim_reward;
        committee_stake_info.can_claim_reward = 0;
        assert_eq!(&Committee::committee_stake(&committee1), &committee_stake_info);
        let current_committee1_balance =
            INIT_BALANCE - 20000 * ONE_DBC + committee_stake_info.claimed_reward;
        assert_eq!(Balances::free_balance(committee1), current_committee1_balance);
        // NOTE: 测试 控制账户重新上线机器
        assert_ok!(OnlineProfile::offline_machine_change_hardware_info(
            RuntimeOrigin::signed(controller),
            machine_id.clone()
        ));

        {
            // - Writes:
            // LiveMachines, MachineInfo, StashStake, UserMutHardwarestake, PosGPUInfo, StashMachine
            // CurrentEraStashPoints, NextEraStashPoints, CurrentEraMachinePoints,
            // NextEraMachinePoints, SysInfo,
            assert_eq!(
                OnlineProfile::live_machines(),
                LiveMachine { bonding_machine: vec![machine_id.clone()], ..Default::default() }
            );
            machine_info.machine_status =
                MachineStatus::StakerReportOffline(8643, Box::new(MachineStatus::Online));
            assert_eq!(OnlineProfile::machines_info(&machine_id), Some(machine_info.clone()));
            // 支付 4% * 400000 DBC = 16000DBC
            assert_eq!(OnlineProfile::stash_stake(&stash), (2000 + 400000 + 16000) * ONE_DBC);
            assert_eq!(
                OnlineProfile::user_mut_hardware_stake(&stash, &machine_id),
                online_profile::UserMutHardwareStakeInfo {
                    verify_fee: 2000 * ONE_DBC,
                    offline_slash: 16000 * ONE_DBC,
                    offline_time: 2880 * 3 + 3,
                    need_fulfilling: false,
                }
            );
            assert_eq!(
                OnlineProfile::pos_gpu_info(Longitude::East(1157894), Latitude::North(235678)),
                online_profile::PosInfo { offline_gpu: 4, ..Default::default() }
            );

            assert_eq!(
                OnlineProfile::sys_info(),
                online_profile::SysInfoDetail {
                    total_stake: (400000 + 2000 + 16000) * ONE_DBC,
                    ..Default::default()
                }
            );
            stash_machine_info.online_machine = vec![];
            stash_machine_info.total_gpu_num = 0;
            stash_machine_info.total_calc_points = 0;
            assert_eq!(&OnlineProfile::stash_machines(&stash), &stash_machine_info);

            // 当前Era为3
            assert_eq!(OnlineProfile::current_era(), 4);
            assert_eq!(OnlineProfile::eras_stash_points(4), EraStashPoints::default());
            assert_eq!(OnlineProfile::eras_stash_points(5), EraStashPoints::default());
            assert_eq!(OnlineProfile::eras_machine_points(4), BTreeMap::new());
            assert_eq!(OnlineProfile::eras_machine_points(5), BTreeMap::new());
        }

        // 控制账户重新添加机器信息
        assert_ok!(OnlineProfile::add_machine_info(
            RuntimeOrigin::signed(controller),
            machine_id.clone(),
            StakerCustomizeInfo {
                server_room: server_room[0],
                upload_net: 100,
                download_net: 100,
                longitude: Longitude::East(1157894),
                latitude: Latitude::North(235678),
                telecom_operators: vec!["China Unicom".into()],
            }
        ));

        {
            assert_eq!(
                OnlineProfile::live_machines(),
                LiveMachine { confirmed_machine: vec![machine_id.clone()], ..Default::default() }
            );
        }

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
            LiveMachine { booked_machine: vec![machine_id.clone()], ..Default::default() }
        );
        let x = OnlineProfile::machines_info(&machine_id);
        machine_info.machine_status = MachineStatus::CommitteeVerifying;
        assert_eq!(OnlineProfile::machines_info(&machine_id), Some(machine_info.clone()));
        assert_eq!(
            OnlineCommittee::machine_committee(machine_id.clone()),
            OCMachineCommitteeList {
                book_time: 2880 * 3 + 3,
                confirm_start_time: 2880 * 3 + 3 + 4320,
                booked_committee: vec![committee2, committee3, committee1],
                hashed_committee: vec![],
                confirmed_committee: vec![],
                onlined_committee: vec![],
                status: Default::default()
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
                verify_time: vec![9603, 11043, 12483],
                ..Default::default()
            }
        );

        // 委员会提交机器Hash
        let machine_info_hash1: [u8; 16] =
            hex::decode("53cf058dfa07ef517b2f28bccff88c2b").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            RuntimeOrigin::signed(committee1),
            machine_id.clone(),
            machine_info_hash1
        ));

        let machine_info_hash2: [u8; 16] =
            hex::decode("3f775d3f4a144b94d6d551f6091a5126").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            RuntimeOrigin::signed(committee2),
            machine_id.clone(),
            machine_info_hash2
        ));

        let machine_info_hash3: [u8; 16] =
            hex::decode("4983040157403addac94ca860ddbff7f").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            RuntimeOrigin::signed(committee3),
            machine_id.clone(),
            machine_info_hash3
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
        assert_ok!(&OnlineCommittee::submit_confirm_raw(
            RuntimeOrigin::signed(committee1),
            committee_upload_info.clone()
        ));
        committee_upload_info.rand_str = "abcdefg2".as_bytes().to_vec();
        assert_ok!(&OnlineCommittee::submit_confirm_raw(
            RuntimeOrigin::signed(committee2),
            committee_upload_info.clone()
        ));
        committee_upload_info.rand_str = "abcdefg3".as_bytes().to_vec();
        assert_ok!(&OnlineCommittee::submit_confirm_raw(
            RuntimeOrigin::signed(committee3),
            committee_upload_info.clone()
        ));

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
                onlined_committee: vec![]
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
                verify_time: vec![9603, 11043, 12483],
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
        // MachinesInfo, LiveMachines, SysInfo, StashStake, ServerRoomMachines, PosGPUInfo,
        // EraMachineSnap, EraStashSnap CommitteeReward(get reward immediately),
        // UserReonlineStake, do Slash

        committee_upload_info.rand_str = vec![];
        machine_info.machine_info_detail.committee_upload_info = committee_upload_info;
        let machine_info = MachineInfo {
            last_machine_restake: 8644,
            last_online_height: 8644,
            stake_amount: 8000 * ONE_DBC,
            machine_status: MachineStatus::Online,
            ..machine_info
        };

        assert_eq!(OnlineProfile::machines_info(&machine_id), Some(machine_info));
        assert_eq!(
            OnlineProfile::live_machines(),
            LiveMachine { online_machine: vec!(machine_id.clone()), ..Default::default() }
        );
        // 之前质押40w 重新上线需要质押8000 不需要补充质押 所有total_stake=40w
        assert_eq!(
            OnlineProfile::sys_info(),
            online_profile::SysInfoDetail {
                total_gpu_num: 8,
                total_staker: 1,
                total_calc_points: 119876, // 119780 + 119780 * 8 /10000 = 119875.824
                total_stake: 400000 * ONE_DBC,
                ..Default::default()
            }
        );
        assert_eq!(
            OnlineProfile::user_mut_hardware_stake(&stash, &machine_id),
            UserMutHardwareStakeInfo { ..Default::default() }
        );
        assert_eq!(OnlineProfile::stash_stake(&stash), 400000 * ONE_DBC);
        // 检查分数

        let mut staker_statistic = BTreeMap::new();
        staker_statistic.insert(
            stash,
            online_profile::StashMachineStatistics {
                online_gpu_num: 8,
                inflation: Perbill::from_rational(8u32, 10000),
                machine_total_calc_point: 119780,
                rent_extra_grade: 0,
            },
        );
        assert_eq!(
            OnlineProfile::eras_stash_points(5),
            EraStashPoints { total: 119876, staker_statistic: staker_statistic.clone() }
        );

        let mut era_machine_points = BTreeMap::new();
        era_machine_points.insert(
            machine_id.clone(),
            MachineGradeStatus { basic_grade: 119780, is_rented: false },
        );
        assert_eq!(OnlineProfile::eras_machine_points(5), era_machine_points);
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
        let committee1 = sr25519::Public::from(Sr25519Keyring::Alice);
        let _committee2 = sr25519::Public::from(Sr25519Keyring::Charlie);
        let committee3 = sr25519::Public::from(Sr25519Keyring::Dave);
        let committee4 = sr25519::Public::from(Sr25519Keyring::Eve);

        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();

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
        let machine_info_hash1 =
            hex::decode("f813d5478a1c6cfb04a203a0643ad67e").unwrap().try_into().unwrap();
        let machine_info_hash2 =
            hex::decode("8beab87415978daf436f31a292f9bdbb").unwrap().try_into().unwrap();

        assert_ok!(OnlineCommittee::submit_confirm_hash(
            RuntimeOrigin::signed(committee1),
            machine_id.clone(),
            machine_info_hash1,
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            RuntimeOrigin::signed(committee3),
            machine_id.clone(),
            machine_info_hash2,
        ));

        let machine_committee = crate::OCMachineCommitteeList {
            book_time: 6,
            booked_committee: vec![committee3, committee1, committee4],
            hashed_committee: vec![committee3, committee1],
            confirm_start_time: 4326,
            confirmed_committee: vec![],
            onlined_committee: vec![],
            status: crate::OCVerifyStatus::default(),
        };

        assert_eq!(OnlineCommittee::machine_committee(&machine_id), machine_committee);

        // 等到36个小时之后，提交确认信息
        run_to_block(4326); // 6 + 36 * 120 = 4326

        // 委员会提交原始信息
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            RuntimeOrigin::signed(committee1),
            CommitteeUploadInfo { rand_str: rand_str1, ..machine_base_info.clone() }
        ));
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            RuntimeOrigin::signed(committee3),
            CommitteeUploadInfo { rand_str: rand_str2, ..machine_base_info }
        ));

        run_to_block(4327);

        assert_eq!(
            OnlineProfile::live_machines(),
            LiveMachine { online_machine: vec![machine_id.clone()], ..Default::default() }
        );

        assert_eq!(
            OnlineCommittee::pending_slash(0),
            Some(OCPendingSlashInfo {
                machine_id,
                inconsistent_committee: vec![],
                unruly_committee: vec![committee4],
                reward_committee: vec![committee3, committee1],
                committee_stake: 1000 * ONE_DBC,
                slash_time: 4327,
                slash_exec_time: 4327 + 2880 * 2,
                book_result: OCBookResultType::OnlineSucceed,
                slash_result: OCSlashResult::Pending,
                machine_stash: None,
                stash_slash_amount: 0
            })
        );

        let committee4_box_pubkey =
            hex::decode("5eec53877f4b18c8b003fa983d27ef2e5518b7e4d08d482922a7787f2ea75529")
                .unwrap()
                .try_into()
                .unwrap();

        assert_eq!(
            Committee::committee_stake(committee4),
            committee::CommitteeStakeInfo {
                box_pubkey: committee4_box_pubkey,
                staked_amount: 20000 * ONE_DBC,
                used_stake: 1000 * ONE_DBC,
                can_claim_reward: 0,
                claimed_reward: 0,
            }
        );

        // 惩罚
        run_to_block(4327 + 2880 * 2 + 1);

        assert_eq!(
            Committee::committee_stake(committee4),
            committee::CommitteeStakeInfo {
                box_pubkey: committee4_box_pubkey,
                staked_amount: 19000 * ONE_DBC,
                used_stake: 0,
                can_claim_reward: 0,
                claimed_reward: 0,
            }
        );
    })
}

// TODO: 三个委员会两个正常工作，一个提交Hash之后，没有提交原始值，检查惩罚机制
#[test]
fn committee_not_wubmit_raw_slash_works() {
    new_test_with_online_machine_distribution().execute_with(|| {
        let _committee1 = sr25519::Public::from(Sr25519Keyring::Alice);
        let _committee2 = sr25519::Public::from(Sr25519Keyring::Charlie);
        let _committee3 = sr25519::Public::from(Sr25519Keyring::Dave);
        let _committee4 = sr25519::Public::from(Sr25519Keyring::Eve);

        let _machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();
    })
}

// 三个委员会提交信息不一致，导致重新分派
#[test]
fn committee_not_equal_then_redistribute_works() {
    new_test_with_init_params_ext().execute_with(|| {
        // Bob pubkey
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();
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

        let committee1 = sr25519::Public::from(Sr25519Keyring::Ferdie);
        let committee2 = sr25519::Public::from(Sr25519Keyring::One);
        let committee3 = sr25519::Public::from(Sr25519Keyring::Two);

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

        let machine_info_hash1: [u8; 16] =
            hex::decode("fd8885a22a9d9784adaa36effcd77522").unwrap().try_into().unwrap();
        let machine_info_hash2: [u8; 16] =
            hex::decode("26c58bca9792cc285aa0a2e42483131b").unwrap().try_into().unwrap();
        let machine_info_hash3: [u8; 16] =
            hex::decode("5745567d193b6d3cba18412489ccd433").unwrap().try_into().unwrap();

        let controller = sr25519::Public::from(Sr25519Keyring::Eve);
        let stash = sr25519::Public::from(Sr25519Keyring::Ferdie);

        // Machine account Info:
        // ❯ subkey generate --scheme sr25519
        //   Secret seed:       0x16f2e4b3ad50aab4f5c7ab56d793738a893080d578976040a5be284da12437b6
        //   Public key (hex):  0xdc763e931919cceee0c35392d124c753fc4e4ab6e494bc67722fdd31989d660f

        committee_upload_info.machine_id = machine_id.clone();

        // stash 账户设置控制账户
        assert_ok!(OnlineProfile::set_controller(RuntimeOrigin::signed(stash), controller));
        // controller 生成server_name
        assert_ok!(OnlineProfile::gen_server_room(RuntimeOrigin::signed(controller)));
        let server_room = OnlineProfile::stash_server_rooms(&stash);
        assert_ok!(OnlineProfile::bond_machine(
            RuntimeOrigin::signed(controller),
            machine_id.clone(),
            msg.as_bytes().to_vec(),
            hex::decode(sig).unwrap()
        ));

        let mut machine_info = MachineInfo {
            controller,
            machine_stash: stash,
            bonding_height: 3,
            stake_amount: 100000 * ONE_DBC,
            machine_status: MachineStatus::AddingCustomizeInfo,
            renters: vec![],
            last_machine_restake: 0,
            online_height: 0,
            last_online_height: 0,
            init_stake_per_gpu: 0,
            total_rented_duration: 0,
            total_rented_times: 0,
            total_rent_fee: 0,
            total_burn_fee: 0,
            machine_info_detail: Default::default(),
            reward_committee: vec![],
            reward_deadline: 0,
        };

        let customize_info = StakerCustomizeInfo {
            server_room: server_room[0],
            upload_net: 100,
            download_net: 100,
            longitude: Longitude::East(1157894),
            latitude: Latitude::North(235678),
            telecom_operators: vec!["China Unicom".into()],
        };
        assert_ok!(OnlineProfile::add_machine_info(
            RuntimeOrigin::signed(controller),
            machine_id.clone(),
            customize_info.clone()
        ));

        machine_info.machine_info_detail.staker_customize_info = customize_info;
        machine_info.machine_status = MachineStatus::DistributingOrder;

        run_to_block(15);

        // 添加三个委员会
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee1));
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee2));
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee3));

        assert_ok!(Committee::committee_set_box_pubkey(
            RuntimeOrigin::signed(committee1),
            committee1_box_pubkey
        ));
        assert_ok!(Committee::committee_set_box_pubkey(
            RuntimeOrigin::signed(committee2),
            committee2_box_pubkey
        ));
        assert_ok!(Committee::committee_set_box_pubkey(
            RuntimeOrigin::signed(committee3),
            committee3_box_pubkey
        ));

        run_to_block(16);
        assert_eq!(
            OnlineProfile::live_machines(),
            LiveMachine { booked_machine: vec![machine_id.clone()], ..Default::default() }
        );

        machine_info.machine_status = MachineStatus::CommitteeVerifying;

        // 正常Hash: 0x6b561dfad171810dfb69924dd68733ec
        // cpu_core_num: 48: 0x5b4499c4b6e9f080673f9573410a103a
        // cpu_core_num: 96: 0x3ac5b3416d1743b58a4c9af58c7002d7

        // 三个委员会分别提交机器Hash
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            RuntimeOrigin::signed(committee1),
            machine_id.clone(),
            machine_info_hash1
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            RuntimeOrigin::signed(committee2),
            machine_id.clone(),
            machine_info_hash2
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            RuntimeOrigin::signed(committee3),
            machine_id.clone(),
            machine_info_hash3
        ));

        // 委员会提交原始信息
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            RuntimeOrigin::signed(committee1),
            committee_upload_info.clone()
        ));
        committee_upload_info.mem_num = 441;
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            RuntimeOrigin::signed(committee2),
            committee_upload_info.clone()
        ));
        committee_upload_info.mem_num = 442;
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            RuntimeOrigin::signed(committee3),
            committee_upload_info
        ));

        assert_eq!(
            OnlineCommittee::machine_committee(&machine_id),
            OCMachineCommitteeList {
                book_time: 16,
                booked_committee: vec![committee3, committee1, committee2],
                hashed_committee: vec![committee3, committee1, committee2],
                confirmed_committee: vec![committee3, committee1, committee2],
                confirm_start_time: 4336,
                status: OCVerifyStatus::Summarizing,
                onlined_committee: vec![]
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
            committee::CommitteeList {
                normal: vec![committee3, committee1, committee2],
                ..Default::default()
            }
        );

        run_to_block(17);

        // 机器直接删掉信息即可
        assert_eq!(
            OnlineCommittee::committee_machine(&committee1),
            OCCommitteeMachineList {
                booked_machine: vec![machine_id.clone()],
                ..Default::default()
            }
        );
        assert_eq!(
            OnlineCommittee::committee_machine(&committee2),
            OCCommitteeMachineList {
                booked_machine: vec![machine_id.clone()],
                ..Default::default()
            }
        );
        assert_eq!(
            OnlineCommittee::committee_machine(&committee3),
            OCCommitteeMachineList {
                booked_machine: vec![machine_id.clone()],
                ..Default::default()
            }
        );

        // NOTE: 如果on_finalize先执行lease_committee 再z执行online_profile则没有内容，否则被重新分配了
        assert_eq!(
            OnlineCommittee::machine_committee(&machine_id),
            OCMachineCommitteeList {
                book_time: 17,
                booked_committee: vec![committee3, committee1, committee2],
                confirm_start_time: 4337,
                hashed_committee: vec![],
                confirmed_committee: vec![],
                onlined_committee: vec![],
                status: Default::default()
            }
        );

        let machine_submit_hash: Vec<[u8; 16]> = vec![];
        assert_eq!(OnlineCommittee::machine_submited_hash(&machine_id), machine_submit_hash);
        assert_eq!(
            OnlineCommittee::committee_ops(&committee1, &machine_id),
            OCCommitteeOps {
                staked_dbc: 1000 * ONE_DBC,
                verify_time: vec![497, 1937, 3377],
                ..Default::default()
            }
        );

        assert_eq!(
            Committee::committee_stake(committee1),
            committee::CommitteeStakeInfo {
                box_pubkey: committee1_box_pubkey,
                staked_amount: 20000 * ONE_DBC,
                // FIXME: 重新分派时，将退还已使用的质押
                used_stake: 1000 * ONE_DBC,
                can_claim_reward: 0,
                claimed_reward: 0,
            }
        );
    })
}

// After 2 submit hash, and it's time to submit raw,
// machine status is still submiting_hash.  (#44)
#[test]
fn two_submit_hash_reach_submit_raw_works() {
    new_test_with_init_params_ext().execute_with(|| {
        // Bob pubkey
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();
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

        let committee1 = sr25519::Public::from(Sr25519Keyring::Ferdie);
        let committee2 = sr25519::Public::from(Sr25519Keyring::One);
        let committee3 = sr25519::Public::from(Sr25519Keyring::Two);

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

        let machine_info_hash1: [u8; 16] =
            hex::decode("fd8885a22a9d9784adaa36effcd77522").unwrap().try_into().unwrap();
        let machine_info_hash2: [u8; 16] =
            hex::decode("c016090e0943c17f5d4999dc6eb52683").unwrap().try_into().unwrap();

        let controller = sr25519::Public::from(Sr25519Keyring::Eve);
        let stash = sr25519::Public::from(Sr25519Keyring::Ferdie);

        committee_upload_info.machine_id = machine_id.clone();

        // stash 账户设置控制账户
        assert_ok!(OnlineProfile::set_controller(RuntimeOrigin::signed(stash), controller));
        // controller 生成server_name
        assert_ok!(OnlineProfile::gen_server_room(RuntimeOrigin::signed(controller)));
        let server_room = OnlineProfile::stash_server_rooms(&stash);
        assert_ok!(OnlineProfile::bond_machine(
            RuntimeOrigin::signed(controller),
            machine_id.clone(),
            msg.as_bytes().to_vec(),
            hex::decode(sig).unwrap()
        ));

        let mut machine_info = MachineInfo {
            controller,
            machine_stash: stash,
            bonding_height: 3,
            stake_amount: 100000 * ONE_DBC,
            machine_status: MachineStatus::AddingCustomizeInfo,
            renters: vec![],
            last_machine_restake: 0,
            online_height: 0,
            last_online_height: 0,
            init_stake_per_gpu: 0,
            total_rented_duration: 0,
            total_rented_times: 0,
            total_rent_fee: 0,
            total_burn_fee: 0,
            machine_info_detail: Default::default(),
            reward_committee: vec![],
            reward_deadline: 0,
        };

        let customize_info = StakerCustomizeInfo {
            server_room: server_room[0],
            upload_net: 100,
            download_net: 100,
            longitude: Longitude::East(1157894),
            latitude: Latitude::North(235678),
            telecom_operators: vec!["China Unicom".into()],
        };
        assert_ok!(OnlineProfile::add_machine_info(
            RuntimeOrigin::signed(controller),
            machine_id.clone(),
            customize_info.clone()
        ));

        machine_info.machine_info_detail.staker_customize_info = customize_info;
        machine_info.machine_status = MachineStatus::DistributingOrder;

        run_to_block(15);

        // 添加三个委员会
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee1));
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee2));
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee3));

        assert_ok!(Committee::committee_set_box_pubkey(
            RuntimeOrigin::signed(committee1),
            committee1_box_pubkey
        ));
        assert_ok!(Committee::committee_set_box_pubkey(
            RuntimeOrigin::signed(committee2),
            committee2_box_pubkey
        ));
        assert_ok!(Committee::committee_set_box_pubkey(
            RuntimeOrigin::signed(committee3),
            committee3_box_pubkey
        ));

        run_to_block(16);
        assert_eq!(
            OnlineProfile::live_machines(),
            LiveMachine { booked_machine: vec![machine_id.clone()], ..Default::default() }
        );

        machine_info.machine_status = MachineStatus::CommitteeVerifying;

        // 正常Hash: 0x6b561dfad171810dfb69924dd68733ec
        // cpu_core_num: 48: 0x5b4499c4b6e9f080673f9573410a103a
        // cpu_core_num: 96: 0x3ac5b3416d1743b58a4c9af58c7002d7

        assert_eq!(
            OnlineCommittee::machine_committee(&machine_id),
            OCMachineCommitteeList {
                book_time: 16,
                booked_committee: vec![committee3, committee1, committee2],
                confirm_start_time: 4336,
                status: OCVerifyStatus::SubmittingHash,
                hashed_committee: vec![],
                confirmed_committee: vec![],
                onlined_committee: vec![]
            }
        );

        // 两个委员会分别提交机器Hash
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            RuntimeOrigin::signed(committee1),
            machine_id.clone(),
            machine_info_hash1
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            RuntimeOrigin::signed(committee2),
            machine_id.clone(),
            machine_info_hash2
        ));

        assert_eq!(
            OnlineCommittee::machine_committee(&machine_id),
            OCMachineCommitteeList {
                book_time: 16,
                booked_committee: vec![committee3, committee1, committee2],
                hashed_committee: vec![committee1, committee2],
                confirm_start_time: 4336,
                status: OCVerifyStatus::SubmittingHash,
                confirmed_committee: vec![],
                onlined_committee: vec![]
            }
        );

        run_to_block(4336);

        assert_eq!(
            OnlineCommittee::machine_committee(&machine_id),
            OCMachineCommitteeList {
                book_time: 16,
                booked_committee: vec![committee3, committee1, committee2],
                hashed_committee: vec![committee1, committee2],
                confirm_start_time: 4336,
                status: OCVerifyStatus::SubmittingRaw,
                confirmed_committee: vec![],
                onlined_committee: vec![]
            }
        );

        // 委员会提交原始信息
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            RuntimeOrigin::signed(committee1),
            committee_upload_info.clone()
        ));
        committee_upload_info.rand_str = "abcdefg2".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            RuntimeOrigin::signed(committee2),
            committee_upload_info
        ));
    })
}
