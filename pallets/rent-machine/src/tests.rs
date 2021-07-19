use crate::mock::*;
use dbc_price_ocw::MAX_LEN;
use frame_support::assert_ok;
use online_profile::{CommitteeUploadInfo, StakerCustomizeInfo, StandardGpuPointPrice};
use std::convert::TryInto;

#[test]
fn rent_machine_should_works() {
    new_test_ext().execute_with(|| {
        run_to_block(1);

        // 上线一台机器
        let _alice: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Alice).into();
        let _bob: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Bob).into();
        let _charile: sp_core::sr25519::Public =
            sr25519::Public::from(Sr25519Keyring::Charlie).into();
        let renter_dave: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Dave).into();

        let one_committee: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One).into();
        let pot_two: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Two).into();

        // Controller
        let controller: sp_core::sr25519::Public =
            sr25519::Public::from(Sr25519Keyring::Eve).into();
        // Stash
        let stash: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();
        // Bob pubkey
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48";
        let machine_id = machine_id.as_bytes().to_vec();

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
        assert_ok!(GenericFunc::set_fixed_tx_fee(RawOrigin::Root.into(), 10 * ONE_DBC));
        // 每张GPU质押数量: 100,000 DBC
        assert_ok!(OnlineProfile::set_gpu_stake(RawOrigin::Root.into(), 100_000 * ONE_DBC));
        // 设置奖励发放开始时间
        assert_ok!(OnlineProfile::set_reward_start_era(RawOrigin::Root.into(), 0));
        // 设置每个Era奖励数量: 1,100,000
        assert_ok!(OnlineProfile::set_phase_n_reward_per_era(
            RawOrigin::Root.into(),
            0,
            1_100_000 * ONE_DBC
        ));
        assert_ok!(OnlineProfile::set_phase_n_reward_per_era(
            RawOrigin::Root.into(),
            1,
            1_100_000 * ONE_DBC
        ));
        // 设置单卡质押上限： 7700_000_000
        assert_ok!(OnlineProfile::set_stake_usd_limit(RawOrigin::Root.into(), 7700_000_000));
        // 设置标准GPU租金价格: (3080得分1000；租金每月1000RMB) {1000; 150_000_000};
        assert_ok!(OnlineProfile::set_standard_gpu_point_price(
            RawOrigin::Root.into(),
            StandardGpuPointPrice { gpu_point: 1000, gpu_price: 150_000_000 }
        ));
        // 设置机器租金支付地址
        assert_ok!(RentMachine::set_rent_fee_pot(RawOrigin::Root.into(), pot_two));

        run_to_block(2);

        // stash 账户设置控制账户
        assert_ok!(OnlineProfile::set_controller(Origin::signed(stash), controller));

        // controller bond_machine
        let msg = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a485CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL";
        // NOTE:  测试中签名不以0x开头
        let sig = "3abb2adb1bad83b87d61be8e55c31cec4b3fb2ecc5ee7254c8df88b1ec92e0254f4a9b010e2d8a5cce9d262e9193b76be87b46f6bef4219517cf939520bfff84";

        assert_ok!(OnlineProfile::bond_machine(
            Origin::signed(controller),
            machine_id.clone(),
            msg.as_bytes().to_vec(),
            hex::decode(sig).unwrap()
        ));

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

        // 增加一个委员会
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), one_committee));
        let one_box_pubkey = hex::decode("9dccbab2d61405084eac440f877a6479bc827373b2e414e81a6170ebe5aadd12").unwrap().try_into().unwrap();
        assert_ok!(Committee::committee_set_box_pubkey(Origin::signed(one_committee), one_box_pubkey));

        run_to_block(5);

        // 委员会提交机器Hash
        let machine_info_hash = "d80b116fd318f19fd89da792aba5e875";
        assert_ok!(LeaseCommittee::submit_confirm_hash(
            Origin::signed(one_committee),
            machine_id.clone(),
            hex::decode(machine_info_hash).unwrap().try_into().unwrap()
        ));

        // 委员会提交原始信息
        assert_ok!(LeaseCommittee::submit_confirm_raw(
            Origin::signed(one_committee),
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

        // dave租用了10天
        assert_ok!(RentMachine::rent_machine(Origin::signed(renter_dave), machine_id.clone(), 10));

        run_to_block(50);

        // dave确认租用成功
        assert_ok!(RentMachine::confirm_rent(Origin::signed(renter_dave), machine_id.clone()));

        // dave续租成功
        assert_ok!(RentMachine::relet_machine(Origin::signed(renter_dave), machine_id.clone(), 10));

        // TODO: 检查机器得分

        // TODO: 检查租金是否正确扣除

        // TODO: 检查机器退租后，状态是否清理

        // TODO: 检查机器没有租用成功，押金正常退回
    })
}
