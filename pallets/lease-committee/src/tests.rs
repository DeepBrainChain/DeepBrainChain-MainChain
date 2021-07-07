#![allow(dead_code)]

use crate::{mock::*, LCMachineCommitteeList};
use committee::CommitteeList;
use dbc_price_ocw::MAX_LEN;
use frame_support::assert_ok;
use online_profile::{
    CommitteeUploadInfo, LiveMachine, StakerCustomizeInfo, StandardGpuPointPrice,
};
use std::convert::TryInto;

#[test]
fn machine_online_works() {
    new_test_ext().execute_with(|| {
        run_to_block(1);

        let alice: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Alice).into();
        let _bob: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Bob).into();
        let _charile: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Charlie).into();
        let _dave: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Dave).into();

        let one: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One).into();
        let _two: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Two).into();

        let controller: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into(); // Controller
        let stash: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into(); // Stash
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"; // Bob pubkey

        assert_eq!(Balances::free_balance(alice), INIT_BALANCE);

        // 初始化price_ocw (0.012$)
        assert_eq!(DBCPriceOCW::avg_price(), None);
        for _ in 0..MAX_LEN {
            DBCPriceOCW::add_price(12_000u64);
        }
        DBCPriceOCW::add_avg_price();
        assert_eq!(DBCPriceOCW::avg_price(), Some(12_000u64));

        // 初始化设置参数
        // 委员会每次抢单质押数量 (15$)
        assert_ok!(Committee::set_staked_usd_per_order(RawOrigin::Root.into(), 15_000_000));
        // 操作时的固定费率: 10 DBC
        assert_ok!(GenericFunc::set_fixed_tx_fee(RawOrigin::Root.into(), 10*ONE_DBC));
        // 每张GPU质押数量: 100,000 DBC
        assert_ok!(OnlineProfile::set_gpu_stake(RawOrigin::Root.into(), 100_000 * ONE_DBC));
        // 设置奖励发放开始时间
        assert_ok!(OnlineProfile::set_reward_start_era(RawOrigin::Root.into(), 0));
        // 设置每个Era奖励数量: 1,100,000
        assert_ok!(OnlineProfile::set_phase_n_reward_per_era(RawOrigin::Root.into(), 0, 1_100_000*ONE_DBC));
        assert_ok!(OnlineProfile::set_phase_n_reward_per_era(RawOrigin::Root.into(), 1, 1_100_000*ONE_DBC));
        // 设置单卡质押上限： 7700_000_000
        assert_ok!(OnlineProfile::set_stake_usd_limit(RawOrigin::Root.into(), 7700_000_000));
        // 设置标准GPU租金价格: (3080得分1000；租金每月1000RMB) {1000; 150_000_000};
        assert_ok!(OnlineProfile::set_standard_gpu_point_price(RawOrigin::Root.into(), StandardGpuPointPrice{gpu_point: 1000, gpu_price: 150_000_000}));

        run_to_block(2);

        // 查询状态
        assert_eq!(Committee::committee_stake_usd_per_order(), Some(15_000_000));
        assert_eq!(Committee::committee_stake_dbc_per_order(), Some(1250 * ONE_DBC)); // 15_000_000 / 12_000 * 10*15 = 1250 DBC

        // stash 账户设置控制账户
        assert_ok!(OnlineProfile::set_controller(Origin::signed(stash), controller));

        // controller bond_machine
        let msg = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a485CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL";
        // NOTE:  测试中签名不以0x开头
        let sig = "3abb2adb1bad83b87d61be8e55c31cec4b3fb2ecc5ee7254c8df88b1ec92e0254f4a9b010e2d8a5cce9d262e9193b76be87b46f6bef4219517cf939520bfff84";

        assert_ok!(OnlineProfile::bond_machine(
            Origin::signed(controller),
            machine_id.as_bytes().to_vec(),
            msg.as_bytes().to_vec(),
            hex::decode(sig).unwrap()
        ));

        // NOTE: 查询bond_machine之后的各种状态
        // 1. LiveMachine
        assert_eq!(OnlineProfile::live_machines(), LiveMachine{
            bonding_machine: vec!(machine_id.as_bytes().to_vec()),
            ..Default::default()
        });
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
        });

        // 控制账户添加机器信息
        assert_ok!(OnlineProfile::add_machine_info(
            Origin::signed(controller),
            machine_id.as_bytes().to_vec(),
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
        assert_eq!(OnlineProfile::live_machines(), LiveMachine{
            confirmed_machine: vec!(machine_id.as_bytes().to_vec()),
            ..Default::default()
        });

        // 增加一个委员会
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), one));
        let one_box_pubkey = hex::decode("9dccbab2d61405084eac440f877a6479bc827373b2e414e81a6170ebe5aadd12").unwrap().try_into().unwrap();
        assert_ok!(Committee::committee_set_box_pubkey(Origin::signed(one), one_box_pubkey));

        // 再增加两个委员会
        // assert_ok!(Committee::add_committee(RawOrigin::Root.into(), two));
        // assert_ok!(Committee::add_committee(RawOrigin::Root.into(), alice));
        // let two_box_pubkey = hex::decode("1e71b5a83ccdeff1592062a1d4da4a272691f08e2024a1ca75a81d534a76210a").unwrap().try_into().unwrap();
        // let alice_box_pubkey = hex::decode("ff3033c763f71bc51f372c1dc5095accc26880e138df84cac13c46bfd7dbd74f").unwrap().try_into().unwrap();
        // assert_ok!(Committee::committee_set_box_pubkey(Origin::signed(two), two_box_pubkey));
        // assert_ok!(Committee::committee_set_box_pubkey(Origin::signed(alice), alice_box_pubkey));

        // 委员会处于正常状态(排序后的列表)
        assert_eq!(Committee::committee(), CommitteeList{normal: vec![one], ..Default::default()});
        // 获取可派单的委员会正常
        assert_ok!(LeaseCommittee::lucky_committee().ok_or(()));

        run_to_block(5);

        // 订单处于正常状态: 已经被委员会预订
        assert_eq!(OnlineProfile::live_machines(), LiveMachine{
            booked_machine: vec!(machine_id.as_bytes().to_vec()),
            ..Default::default()
        });

        // 查询机器中有订阅的委员会
        assert_eq!(
            LeaseCommittee::machine_committee(machine_id.as_bytes().to_vec()),
            LCMachineCommitteeList{
                book_time: 4,
                confirm_start_time: 4324,
                booked_committee: vec![one],
                ..Default::default()}
        );

        // 委员会提交机器Hash
        let machine_info_hash = "d80b116fd318f19fd89da792aba5e875";
        assert_ok!(LeaseCommittee::submit_confirm_hash(
            Origin::signed(one),
            machine_id.as_bytes().to_vec(),
            hex::decode(machine_info_hash).unwrap().try_into().unwrap()
        ));

        // 委员会提交原始信息
        assert_ok!(LeaseCommittee::submit_confirm_raw(
            Origin::signed(one),
            CommitteeUploadInfo {
                machine_id: machine_id.as_bytes().to_vec(),
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
        assert_eq!(OnlineProfile::live_machines(), LiveMachine{
            online_machine: vec!(machine_id.as_bytes().to_vec()),
            ..Default::default()
        });

        // 检查EraMachinePoints
        assert_eq!(
            OnlineProfile::eras_machine_points(0).unwrap(),
            online_profile::EraMachinePoints{..Default::default()}
        );

        // FIXME 验证Era1奖励数量
        // let era_1_era_points = OnlineProfile::eras_machine_points(1).unwrap();
        // assert_eq!(
        //     era_1_era_points,
        //     online_profile::EraMachinePoints{..Default::default()}
        // );

        // 过一个Era: 一天是2880个块
        run_to_block(2880 * 2 + 2);
        // assert_eq!(
        //     OnlineProfile::eras_machine_points(0).unwrap(),
        //     online_profile::EraMachinePoints{..Default::default()}
        // );

        // assert_eq!(
        //     OnlineProfile::eras_machine_points(1).unwrap(),
        //     online_profile::EraMachinePoints{..Default::default()}
        // );

        // 第二个Era矿工查询奖励
        // let stash_machine = OnlineProfile::stash_machines(&stash);
        // assert_eq!(stash_machine.)
        assert_eq!(
            OnlineProfile::stash_machines(&stash),
            online_profile::StashMachine{
                total_machine: vec![machine_id.as_bytes().to_vec()],
                online_machine: vec![machine_id.as_bytes().to_vec()],
                total_calc_points: 6825,
                total_gpu_num: 4,
                total_rented_gpu: 0,
                total_claimed_reward: 0,
                can_claim_reward: 272250*ONE_DBC, // 1100000 * 99% * 25% = 272250 DBC

                left_reward: vec![816750*ONE_DBC].into_iter().collect(), // 1100000 * 99% * 25% = 816750 DBC
                total_rent_fee: 0,
                total_burn_fee: 0,

                ..Default::default()
            }
        );

        // 委员会查询奖励
        assert_eq!(Committee::committee_reward(one).unwrap(), 11000*ONE_DBC); // 1100000 * 1%

        run_to_block(2880 * 3 + 2);
        // 线性释放
        assert_eq!(
            OnlineProfile::stash_machines(&stash),
            online_profile::StashMachine{
                total_machine: vec![machine_id.as_bytes().to_vec()],
                online_machine: vec![machine_id.as_bytes().to_vec()],
                total_calc_points: 6825,
                total_gpu_num: 4,
                total_rented_gpu: 0,
                total_claimed_reward: 0,
                can_claim_reward: 549944999455500000000, // 272250 + 272250 + 816750 / 150 = 549945.0

                left_reward: vec![816750*ONE_DBC, 816750*ONE_DBC].into_iter().collect(),
                total_rent_fee: 0,
                total_burn_fee: 0,

                ..Default::default()
            }
        );

        // 委员会查询奖励
        assert_eq!(Committee::committee_reward(one).unwrap(), 22000*ONE_DBC);

        // 矿工领取奖励
        assert_ok!(OnlineProfile::claim_rewards(Origin::signed(controller)));
        // 领取奖励后，查询剩余奖励
        assert_eq!(
            OnlineProfile::stash_machines(&stash),
            online_profile::StashMachine {
                total_machine: vec![machine_id.as_bytes().to_vec()],
                online_machine: vec![machine_id.as_bytes().to_vec()],
                total_calc_points: 6825,
                total_gpu_num: 4,
                total_rented_gpu: 0,
                total_claimed_reward: 549944999455500000000,
                can_claim_reward: 0,

                left_reward: vec![816750*ONE_DBC, 816750*ONE_DBC].into_iter().collect(),
                total_rent_fee: 0,
                total_burn_fee: 0,

                ..Default::default()
            }
        );
        // 领取奖励后，查询账户余额
        assert_eq!(Balances::free_balance(stash), INIT_BALANCE + 549944999455500000000);

        // 委员会领取奖励
        assert_ok!(Committee::claim_reward(Origin::signed(one)));
        assert_eq!(Balances::free_balance(one), INIT_BALANCE + 22000*ONE_DBC);

        // TODO: Rent machine

        // TODO: report machine
    });
}
