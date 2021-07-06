#![allow(dead_code)]

use crate::{mock::*, LCMachineCommitteeList};
use committee::CommitteeList;
use dbc_price_ocw::MAX_LEN;
use frame_support::assert_ok;
use online_profile::{LiveMachine, StakerCustomizeInfo, StandardGpuPointPrice};
use std::convert::TryInto;

#[test]
#[rustfmt::skip]
fn machine_online_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1); // 随机函数需要初始化

        let alice: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Alice).into();
        let bob: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Bob).into();
        let charile: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Charlie).into();
        let dave: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Dave).into();

        let one: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One).into();
        let two: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Two).into();

        let controller: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into(); // Controller
        let stash: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into(); // Stash
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"; // Bob pubkey

        assert_eq!(Balances::free_balance(alice), 1000_000);

        // 初始化price_ocw (0.012$)
        assert_eq!(DBCPriceOCW::avg_price(), None);
        for _ in 0..MAX_LEN {
            DBCPriceOCW::add_price(12_000u64);
        }
        DBCPriceOCW::add_avg_price();
        assert_eq!(DBCPriceOCW::avg_price(), Some(12_000u64));

        // 初始化设置参数
        // 委员会每次抢单质押数量 (15$)
        assert_ok!(Committee::set_staked_usd_per_order(RawOrigin::Root.into(), 15_000_000u32.into()));
        // 操作时的固定费率
        assert_ok!(GenericFunc::set_fixed_tx_fee(RawOrigin::Root.into(), 10u32.into()));
        // 每张GPU质押数量
        assert_ok!(OnlineProfile::set_gpu_stake(RawOrigin::Root.into(), 100_000u32.into()));
        // 设置奖励发放开始时间
        assert_ok!(OnlineProfile::set_reward_start_era(RawOrigin::Root.into(), 0u32));
        // 设置每个Era奖励数量
        assert_ok!(OnlineProfile::set_phase_n_reward_per_era(RawOrigin::Root.into(), 0, 1_000_000u32.into()));
        assert_ok!(OnlineProfile::set_phase_n_reward_per_era(RawOrigin::Root.into(), 1, 1_000_000u32.into()));
        // 设置单卡质押上限：7_700_000_000
        assert_ok!(OnlineProfile::set_stake_usd_limit(RawOrigin::Root.into(), 7_700_000_000u64.into()));
        // 设置标准GPU租金价格
        assert_ok!(OnlineProfile::set_standard_gpu_point_price(RawOrigin::Root.into(), StandardGpuPointPrice{gpu_point: 1000, gpu_price: 77000000}));

        run_to_block(2);

        // 查询状态
        assert_eq!(DbcPrice::get_dbc_amount_by_value(123), Some(123u64.into()));
        assert_eq!(Committee::committee_stake_usd_per_order(), Some(15_000_000));
        assert_eq!(Committee::committee_stake_dbc_per_order(), Some(1000));

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

        assert_ok!(OnlineProfile::add_machine_info(
            Origin::signed(controller),
            machine_id.as_bytes().to_vec(),
            online_profile::StakerCustomizeInfo {
                upload_net: 1234,
                download_net: 1101,
                longitude: 1112,
                latitude: 2223,
                ..Default::default()}
        ));

        run_to_block(3);
        // 订单处于正常状态
        assert_eq!(OnlineProfile::live_machines(), LiveMachine{
            confirmed_machine: vec!(machine_id.as_bytes().to_vec()),
            ..Default::default()
        });

        // 增加三个委员会
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), one));
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), two));
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), alice));

        // 委员会提交box_pubkey
        let one_box_pubkey = hex::decode("9dccbab2d61405084eac440f877a6479bc827373b2e414e81a6170ebe5aadd12").unwrap().try_into().unwrap();
        let two_box_pubkey = hex::decode("1e71b5a83ccdeff1592062a1d4da4a272691f08e2024a1ca75a81d534a76210a").unwrap().try_into().unwrap();
        let alice_box_pubkey = hex::decode("ff3033c763f71bc51f372c1dc5095accc26880e138df84cac13c46bfd7dbd74f").unwrap().try_into().unwrap();
        assert_ok!(Committee::committee_set_box_pubkey(Origin::signed(one), one_box_pubkey));
        assert_ok!(Committee::committee_set_box_pubkey(Origin::signed(two), two_box_pubkey));
        assert_ok!(Committee::committee_set_box_pubkey(Origin::signed(alice), alice_box_pubkey));

        // 委员会处于正常状态(排序后的列表)
        assert_eq!(Committee::committee(), CommitteeList{normal: vec!(two, one, alice), ..Default::default()});

        // 获取可派单的委员会正常
        assert_ok!(LeaseCommittee::lucky_committee().ok_or(()));




        assert_ok!(LeaseCommittee::distribute_one_machine(&machine_id.as_bytes().to_vec()));

        run_to_block(5);

        // 订单处于正常状态
        assert_eq!(OnlineProfile::live_machines(), LiveMachine{
            confirmed_machine: vec!(machine_id.as_bytes().to_vec()),
            ..Default::default()
        });

        // TODO: 过几个块


        // LeaseCommittee::distribute_machines();

        // 订单处于正常状态
        assert_eq!(OnlineProfile::live_machines(), LiveMachine{
            confirmed_machine: vec!(machine_id.as_bytes().to_vec()),
            ..Default::default()
        });

        run_to_block(10);
        // FIXME
        assert_eq!(
            LeaseCommittee::machine_committee(machine_id.as_bytes().to_vec()),
            LCMachineCommitteeList{..Default::default()}
        );

        // 委员会分配订单

        // 委员会提交机器hash

        // 委员会提交原始信息

        //
    });
}

#[test]
fn select_committee_works() {
    // 质押--参加选举--当选
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let alice: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Alice).into();
        let bob: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Bob).into();
        let charile: sp_core::sr25519::Public =
            sr25519::Public::from(Sr25519Keyring::Charlie).into();
        let dave: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Dave).into();
        let eve: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();

        assert_eq!(Balances::free_balance(alice), 1000_000);

        // // 设置初始值
        // let _ = LeaseCommittee::set_min_stake(RawOrigin::Root.into(), 500_000u32.into());
        // let _ = LeaseCommittee::set_alternate_committee_limit(RawOrigin::Root.into(), 5u32);
        // let _ = LeaseCommittee::set_committee_limit(RawOrigin::Root.into(), 3u32);

        // // 参加选举，成为候选人
        // assert_ok!(LeaseCommittee::stake_for_alternate_committee(
        //     Origin::signed(alice),
        //     500_000u32.into()
        // ));
        // assert_ok!(LeaseCommittee::stake_for_alternate_committee(
        //     Origin::signed(bob),
        //     500_000u32.into()
        // ));
        // assert_ok!(LeaseCommittee::stake_for_alternate_committee(
        //     Origin::signed(charile),
        //     500_000u32.into()
        // ));
        // assert_ok!(LeaseCommittee::stake_for_alternate_committee(
        //     Origin::signed(dave),
        //     500_000u32.into()
        // ));
        // assert_ok!(LeaseCommittee::stake_for_alternate_committee(
        //     Origin::signed(eve),
        //     500_000u32.into()
        // ));

        // assert_eq!(LeaseCommittee::alternate_committee().len(), 5);
        // assert_ok!(LeaseCommittee::reelection_committee(RawOrigin::Root.into()));

        // assert_eq!(LeaseCommittee::committee().len(), 3);
        // assert_eq!(LeaseCommittee::alternate_committee().len(), 5);
    })
}

#[test]
fn book_one_machine_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
    })
}

#[test]
fn bool_all_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
    })
}
