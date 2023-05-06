use super::super::mock::*;
use crate::MachineFaultType;
use frame_support::assert_ok;
use std::convert::TryInto;

// use once_cell::sync::Lazy;
// use online_profile::{EraStashPoints, LiveMachine, StashMachine, SysInfoDetail};
// use rent_machine::{ConfirmingOrder, MachineGPUOrder, RentOrderDetail, RentOrderId, RentStatus};

// const controller: Lazy<sp_core::sr25519::Public> = Lazy::new(||
// sr25519::Public::from(Sr25519Keyring::Eve)); const committee1: Lazy<sp_core::sr25519::Public> =
// Lazy::new(|| sr25519::Public::from(Sr25519Keyring::One)); const committee2:
// Lazy<sp_core::sr25519::Public> = Lazy::new(|| sr25519::Public::from(Sr25519Keyring::Two));
// const committee3: Lazy<sp_core::sr25519::Public> = Lazy::new(||
// sr25519::Public::from(Sr25519Keyring::Ferdie)); const reporter: Lazy<sp_core::sr25519::Public> =
// committee2;

// TODO: 增加15min每确认租用，清理状态后正常

pub fn new_test_with_machine_two_renter() -> sp_io::TestExternalities {
    let mut ext = new_test_with_init_machine_online();

    let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
        .as_bytes()
        .to_vec();
    let renter1 = sr25519::Public::from(Sr25519Keyring::Alice);
    let renter2 = sr25519::Public::from(Sr25519Keyring::Bob);

    ext.execute_with(|| {
        assert_ok!(RentMachine::rent_machine(
            RuntimeOrigin::signed(renter1),
            machine_id.clone(),
            2,
            1 * 2880
        ));
        assert_ok!(RentMachine::rent_machine(
            RuntimeOrigin::signed(renter2),
            machine_id.clone(),
            2,
            1 * 2880
        ));

        assert_ok!(RentMachine::confirm_rent(RuntimeOrigin::signed(renter1), 0));
        assert_ok!(RentMachine::confirm_rent(RuntimeOrigin::signed(renter2), 1));
    });

    ext
}

// 机器有两个订单，分别租用了2个GPU，都是租用状态
// 举报成功之后，两个订单都变成"因举报下架"
#[test]
fn report_individual_gpu_inaccessible() {
    let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
        .as_bytes()
        .to_vec();
    let renter1 = sr25519::Public::from(Sr25519Keyring::Alice);
    // let renter2 = sr25519::Public::from(Sr25519Keyring::Bob);

    let committee = sr25519::Public::from(Sr25519Keyring::One);

    new_test_with_machine_two_renter().execute_with(|| {
        assert_ok!(MaintainCommittee::report_machine_fault(
            RuntimeOrigin::signed(renter1),
            MachineFaultType::RentedInaccessible(machine_id.clone(), 0)
        ));

        // 委员会订阅机器故障报告
        assert_ok!(MaintainCommittee::committee_book_report(RuntimeOrigin::signed(committee), 0));

        // 委员会提交Hash: 内容为 订单ID + 验证人自己的随机数 + 机器是否有问题
        // hash(0abcd1) => 0x73124a023f585b4018b9ed3593c7470a
        let offline_committee_hash: [u8; 16] =
            hex::decode("73124a023f585b4018b9ed3593c7470a").unwrap().try_into().unwrap();
        // - Writes:
        // LiveReport, CommitteeOps, CommitteeOrder, ReportInfo
        assert_ok!(MaintainCommittee::committee_submit_verify_hash(
            RuntimeOrigin::signed(committee),
            0,
            offline_committee_hash.clone()
        ));

        run_to_block(21);

        // - Writes:
        // ReportInfo, committee_ops,
        assert_ok!(MaintainCommittee::committee_submit_inaccessible_raw(
            RuntimeOrigin::signed(committee),
            0,
            "abcd".as_bytes().to_vec(),
            true
        ));

        run_to_block(23);

        // 检查summary结果
        // TODO: 两个订单都是结束的状态
        {}

        // run_to_block(2880 * 5);
        // assert_eq!(1, 2);
    })
}

#[test]
fn report_individual_gpu_fault() {
    new_test_with_machine_two_renter().execute_with(|| {
        run_to_block(2880 * 5);
        // assert_eq!(1, 2);
    })
}
