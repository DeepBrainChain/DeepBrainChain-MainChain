#![allow(dead_code)]

use crate::{mock::*, LCMachineCommitteeList, LCVerifyStatus};
use committee::CommitteeList;
use frame_support::assert_ok;
use online_profile::{CommitteeUploadInfo, LiveMachine, StakerCustomizeInfo};
use std::convert::TryInto;

#[test]
fn machine_online_works() {
    new_test_with_init_params_ext().execute_with(|| {
        let committee1: sp_core::sr25519::Public =
            sr25519::Public::from(Sr25519Keyring::One).into();

        let controller: sp_core::sr25519::Public =
            sr25519::Public::from(Sr25519Keyring::Eve).into();
        let stash: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();
        // Bob pubkey
        let machine_id =
            "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();
        let msg = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48\
                   5CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL";
        let sig = "3abb2adb1bad83b87d61be8e55c31cec4b3fb2ecc5ee7254c8df88b1ec92e025\
                   4f4a9b010e2d8a5cce9d262e9193b76be87b46f6bef4219517cf939520bfff84";

        // 查询状态
        assert_eq!(Balances::free_balance(committee1), INIT_BALANCE);
        assert_eq!(DBCPriceOCW::avg_price(), Some(12_000u64));
        assert_eq!(Committee::committee_stake_usd_per_order(), Some(15_000_000));
        assert_eq!(Committee::committee_stake_dbc_per_order(), Some(1250 * ONE_DBC)); // 15_000_000 / 12_000 * 10*15 = 1250 DBC

        // stash 账户设置控制账户
        assert_ok!(OnlineProfile::set_controller(Origin::signed(stash), controller));
        assert_ok!(OnlineProfile::bond_machine(
            Origin::signed(controller),
            machine_id.clone(),
            msg.as_bytes().to_vec(),
            hex::decode(sig).unwrap()
        ));

        // 查询bond_machine之后的各种状态
        // 1. LiveMachine
        assert_eq!(
            OnlineProfile::live_machines(),
            LiveMachine { bonding_machine: vec!(machine_id.clone()), ..Default::default() }
        );
        // 2. 查询Controller支付10 DBC手续费
        assert_eq!(Balances::free_balance(controller), INIT_BALANCE - 10 * ONE_DBC);
        // 3. 查询Stash质押数量: 10wDBC
        assert_eq!(Committee::user_total_stake(stash).unwrap(), 100_000 * ONE_DBC);
        // 4. 查询stash_machine的信息
        // 5. 查询controller_machine信息
        // 6. 查询MachineInfo
        // 7. 查询系统总质押SysInfo
        assert_eq!(
            OnlineProfile::sys_info(),
            online_profile::SysInfoDetail {
                total_gpu_num: 0,
                total_staker: 0,
                total_calc_points: 0,
                total_stake: 100000 * ONE_DBC,
                ..Default::default()
            }
        );

        // 控制账户添加机器信息
        assert_ok!(OnlineProfile::add_machine_info(
            Origin::signed(controller),
            machine_id.clone(),
            StakerCustomizeInfo {
                upload_net: 10000,
                download_net: 10000,
                longitude: 1157894,
                latitude: 235678,
                telecom_operators: vec!["China Unicom".into()],
                images: vec!["Ubuntu18.04 LTS".into()],
            }
        ));

        run_to_block(3);
        // 订单处于正常状态
        assert_eq!(
            OnlineProfile::live_machines(),
            LiveMachine { confirmed_machine: vec!(machine_id.clone()), ..Default::default() }
        );

        // 增加一个委员会
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee1));
        let one_box_pubkey =
            hex::decode("9dccbab2d61405084eac440f877a6479bc827373b2e414e81a6170ebe5aadd12")
                .unwrap()
                .try_into()
                .unwrap();
        assert_ok!(Committee::committee_set_box_pubkey(Origin::signed(committee1), one_box_pubkey));

        // 委员会处于正常状态(排序后的列表)
        assert_eq!(
            Committee::committee(),
            CommitteeList { normal: vec![committee1], ..Default::default() }
        );
        // 获取可派单的委员会正常
        assert_ok!(LeaseCommittee::lucky_committee().ok_or(()));

        run_to_block(5);

        // 订单处于正常状态: 已经被委员会预订
        assert_eq!(
            OnlineProfile::live_machines(),
            LiveMachine { booked_machine: vec!(machine_id.clone()), ..Default::default() }
        );

        // 查询机器中有订阅的委员会
        assert_eq!(
            LeaseCommittee::machine_committee(machine_id.clone()),
            LCMachineCommitteeList {
                book_time: 4,
                confirm_start_time: 4324,
                booked_committee: vec![committee1],
                ..Default::default()
            }
        );

        // 委员会提交机器Hash
        let machine_info_hash = "d80b116fd318f19fd89da792aba5e875";
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
            }
        ));

        run_to_block(10);

        // 检查机器状态
        assert_eq!(
            OnlineProfile::live_machines(),
            LiveMachine { online_machine: vec!(machine_id.clone()), ..Default::default() }
        );

        // 过一个Era: 一天是2880个块
        run_to_block(2880 * 2 + 2);

        // 第二个Era矿工查询奖励
        assert_eq!(
            OnlineProfile::stash_machines(&stash),
            online_profile::StashMachine {
                total_machine: vec![machine_id.clone()],
                online_machine: vec![machine_id.clone()],
                total_calc_points: 6825,
                total_gpu_num: 4,
                total_rented_gpu: 0,
                total_claimed_reward: 0,
                can_claim_reward: 272250 * ONE_DBC, // (1100000 * 25% * 99% = 272250 DBC) * 2 + (825000 * 1/150 * 0.99 = 544.5) = 545044.5

                linear_release_reward: vec![0, 825_000 * ONE_DBC].into_iter().collect(), // 1100000 * 75% = 8250000 DBC
                total_rent_fee: 0,
                total_burn_fee: 0,

                ..Default::default()
            }
        );

        // 委员会查询奖励
        assert_eq!(Committee::committee_reward(committee1).unwrap(), 2750 * ONE_DBC); // 110_0000 * 25% * 0.1 = 27500

        run_to_block(2880 * 3 + 2);

        // 线性释放
        assert_eq!(
            OnlineProfile::stash_machines(&stash),
            online_profile::StashMachine {
                total_machine: vec![machine_id.clone()],
                online_machine: vec![machine_id.clone()],
                total_calc_points: 6825,
                total_gpu_num: 4,
                total_rented_gpu: 0,
                total_claimed_reward: 0,
                can_claim_reward: 549944999455500000000, // (1100000 * 25% * 99% = 272250 DBC) * 2 + (825000 * 1/150 * 0.99 = 825000 * 6666666/10**9 * 0.99 = 5444.9994555 * 10^15 ) = 549944.9994555 // 相差 0.0005444

                linear_release_reward: vec![0, 825_000 * ONE_DBC, 825_000 * ONE_DBC]
                    .into_iter()
                    .collect(),
                total_rent_fee: 0,
                total_burn_fee: 0,

                ..Default::default()
            }
        );

        // 委员会查询奖励
        assert_eq!(Committee::committee_reward(committee1).unwrap(), 5554999994500000000); // 委员会奖励： (1100000 * 25% * 1% = 2750) * 2 +  (825000 * 6666666/10**9 * 0.01 = 54.9999945) = 5554.9999945 DBC

        // 矿工领取奖励
        assert_ok!(OnlineProfile::claim_rewards(Origin::signed(controller)));
        // 领取奖励后，查询剩余奖励
        assert_eq!(
            OnlineProfile::stash_machines(&stash),
            online_profile::StashMachine {
                total_machine: vec![machine_id.clone()],
                online_machine: vec![machine_id.clone()],
                total_calc_points: 6825,
                total_gpu_num: 4,
                total_rented_gpu: 0,
                total_claimed_reward: 549944999455500000000,
                can_claim_reward: 0,

                linear_release_reward: vec![0, 825_000 * ONE_DBC, 825_000 * ONE_DBC]
                    .into_iter()
                    .collect(),
                total_rent_fee: 0,
                total_burn_fee: 0,

                ..Default::default()
            }
        );
        // 领取奖励后，查询账户余额
        assert_eq!(Balances::free_balance(stash), INIT_BALANCE + 549944999455500000000);

        // 委员会领取奖励
        assert_ok!(Committee::claim_reward(Origin::signed(committee1)));
        assert_eq!(Balances::free_balance(committee1), INIT_BALANCE + 5554999994500000000);

        // TODO: 检查惩罚逻辑

        // TODO: 检查没有成功上线

        // TODO: 检查机器下线后，150天后存储清理
    });
}

// 测试多个委员会分派，全部提交hash，原始信息后,奖励正常，惩罚正常
#[test]
fn all_committee_submit_works() {
    new_test_with_online_machine_distribution().execute_with(|| {
        let _committee1: sp_core::sr25519::Public =
            sr25519::Public::from(Sr25519Keyring::Alice).into();
        let committee2: sp_core::sr25519::Public =
            sr25519::Public::from(Sr25519Keyring::Charlie).into();
        let committee3: sp_core::sr25519::Public =
            sr25519::Public::from(Sr25519Keyring::Dave).into();
        let committee4: sp_core::sr25519::Public =
            sr25519::Public::from(Sr25519Keyring::Eve).into();

        let machine_id =
            "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();

        // 查询机器中对应的委员会列表
        assert_eq!(
            LeaseCommittee::machine_committee(machine_id.clone()),
            LCMachineCommitteeList {
                book_time: 6,
                booked_committee: vec![committee3, committee2, committee4], // Test中非真随机
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

        let rand_str3 = "abcdefg1".as_bytes().to_vec();
        let rand_str2 = "abcdefg2".as_bytes().to_vec();
        let rand_str4 = "abcdefg3".as_bytes().to_vec();

        // 委员会提交机器Hash
        let machine_info_hash3 =
            hex::decode("f813d5478a1c6cfb04a203a0643ad67e").unwrap().try_into().unwrap();
        let machine_info_hash2 =
            hex::decode("8beab87415978daf436f31a292f9bdbb").unwrap().try_into().unwrap();
        let machine_info_hash4 =
            hex::decode("b76f264dbfeba1c25fb0518ed156ab40").unwrap().try_into().unwrap();

        assert_ok!(LeaseCommittee::submit_confirm_hash(
            Origin::signed(committee3),
            machine_id.clone(),
            machine_info_hash3,
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
            Origin::signed(committee3),
            CommitteeUploadInfo { rand_str: rand_str3, ..machine_base_info.clone() }
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
        let _committee1: sp_core::sr25519::Public =
            sr25519::Public::from(Sr25519Keyring::Alice).into();
        let committee2: sp_core::sr25519::Public =
            sr25519::Public::from(Sr25519Keyring::Charlie).into();
        let committee3: sp_core::sr25519::Public =
            sr25519::Public::from(Sr25519Keyring::Dave).into();
        let committee4: sp_core::sr25519::Public =
            sr25519::Public::from(Sr25519Keyring::Eve).into();

        let machine_id =
            "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();
    })
}

// TODO: 三个委员会两个正常工作，一个提交Hash之后，没有提交原始值，检查惩罚机制
fn committee_not_wubmit_raw_slash_works() {
    new_test_with_online_machine_distribution().execute_with(|| {
        let _committee1: sp_core::sr25519::Public =
            sr25519::Public::from(Sr25519Keyring::Alice).into();
        let committee2: sp_core::sr25519::Public =
            sr25519::Public::from(Sr25519Keyring::Charlie).into();
        let committee3: sp_core::sr25519::Public =
            sr25519::Public::from(Sr25519Keyring::Dave).into();
        let committee4: sp_core::sr25519::Public =
            sr25519::Public::from(Sr25519Keyring::Eve).into();

        let machine_id =
            "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();
    })
}

#[test]
fn fulfill_should_work() {
    new_test_with_online_machine_distribution().execute_with(|| {})
}
