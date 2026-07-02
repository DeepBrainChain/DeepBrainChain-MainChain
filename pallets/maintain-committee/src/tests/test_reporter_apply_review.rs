use super::super::mock::*;
use dbc_support::{live_machine::LiveMachine, machine_type::MachineStatus, ONE_DAY, ONE_MINUTE};
use frame_support::assert_ok;
use std::convert::TryInto;

// case1: 报告inaccessible成功后，stash进行申述->申述成功;
// case1.1 申述失败(技术委员会没有进行处理)
// case2: 报告其他错误失败后，报告人进行申述 -> 申述成功
// case2.1 申述失败
// case3: 报告其他错误成功后，stash进行申述
// case4: 报告其他错误失败后，报告人进行申述

// 1个委员会举报成功后
fn after_report_machine_inaccessible() -> sp_io::TestExternalities {
    let mut ext = new_test_with_init_params_ext();
    ext.execute_with(|| {
        let committee = sr25519::Public::from(Sr25519Keyring::One).into();
        let reporter = sr25519::Public::from(Sr25519Keyring::Two).into();
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();

        // 记录：ReportInfo, LiveReport, ReporterReport 并支付处理所需的金额
        assert_ok!(MaintainCommittee::report_machine_fault(
            RuntimeOrigin::signed(reporter),
            crate::MachineFaultType::RentedInaccessible(machine_id.clone(), 0),
        ));

        // 委员会订阅机器故障报告
        assert_ok!(MaintainCommittee::committee_book_report(RuntimeOrigin::signed(committee), 0));

        // 委员会首先提交Hash: 内容为 订单ID + 验证人自己的随机数 + 机器是否有问题
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

        run_to_block(11 + 5 * ONE_MINUTE);
        // - Writes:
        // ReportInfo, committee_ops,
        assert_ok!(MaintainCommittee::committee_submit_inaccessible_raw(
            RuntimeOrigin::signed(committee),
            0,
            "abcd".as_bytes().to_vec(),
            true
        ));

        run_to_block(13 + 5 * ONE_MINUTE);
    });
    ext
}

fn after_report_machine_inaccessible1() -> sp_io::TestExternalities {
    let mut ext = new_test_with_init_params_ext_1();
    ext.execute_with(|| {
        let committee = sr25519::Public::from(Sr25519Keyring::One).into();
        let reporter = sr25519::Public::from(Sr25519Keyring::Eve).into();
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();

        // 记录：ReportInfo, LiveReport, ReporterReport 并支付处理所需的金额
        assert_ok!(MaintainCommittee::report_machine_fault(
            RuntimeOrigin::signed(reporter),
            crate::MachineFaultType::RentedInaccessible(machine_id.clone(), 1),
        ));

        // 委员会订阅机器故障报告
        assert_ok!(MaintainCommittee::committee_book_report(RuntimeOrigin::signed(committee), 0));

        // 委员会首先提交Hash: 内容为 订单ID + 验证人自己的随机数 + 机器是否有问题
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

        run_to_block(11 + 5 * ONE_MINUTE);
        // - Writes:
        // ReportInfo, committee_ops,
        assert_ok!(MaintainCommittee::committee_submit_inaccessible_raw(
            RuntimeOrigin::signed(committee),
            0,
            "abcd".as_bytes().to_vec(),
            true
        ));

        run_to_block(13 + 5 * ONE_MINUTE);
    });
    ext
}

// satsh_apply_slash_after_inaccessible_report
#[test]
fn apply_slash_review_case1() {
    after_report_machine_inaccessible().execute_with(|| {
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();
        let controller = sr25519::Public::from(Sr25519Keyring::Eve).into();

        // [Thread B ③] 委员会 inaccessible 举报路径「保留不触发」：健康检测(DDN)取代举报人机制后，
        //   在租机器「不可达」不再罚 stake bond（RentedInaccessible slash → 0）。委员会验证流程仍跑完，
        //   但不产生 PendingSlash；在租离线的处置改由检测器/自报路径的 ③ 终止逻辑负责，此委员会路径 dormant
        //   （不终止租约）。TODO：slash-申诉机制(apply_slash_review/do_cancel_slash)原仅由本 inaccessible 用例
        //   覆盖，随 slash 归零一并移除；硬件故障 slash 仍会产生罚单(见 test_report_fault_fulfilling_works)，
        //   申诉机制可后续针对硬件故障补测。
        assert_eq!(
            &OnlineProfile::live_machines(),
            &LiveMachine { offline_machine: vec![machine_id.clone()], ..Default::default() }
        );

        assert_ok!(OnlineProfile::controller_report_online(
            RuntimeOrigin::signed(controller),
            machine_id.clone()
        ));

        // 不可达 → 0 stake 罚 → 无 PendingSlash；委员会路径 dormant 不终止租约 → 恢复上线回 Rented
        assert_eq!(OnlineProfile::pending_slash(0), None);
        let machine_info = OnlineProfile::machines_info(&machine_id).unwrap();
        assert_eq!(machine_info.machine_status, MachineStatus::Rented);
    })
}

// satsh_apply_slash_after_inaccessible_report
#[test]
fn apply_slash_review_case1_1() {
    after_report_machine_inaccessible().execute_with(|| {
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();
        let machine_stash = sr25519::Public::from(Sr25519Keyring::Ferdie).into();
        let controller = sr25519::Public::from(Sr25519Keyring::Eve).into();

        // [Thread B ③] 同 case1：inaccessible → 0 stake 罚 → 无 PendingSlash，申诉无从触发。
        assert_ok!(OnlineProfile::controller_report_online(
            RuntimeOrigin::signed(controller),
            machine_id.clone()
        ));

        assert_eq!(OnlineProfile::pending_slash(0), None);
        assert_eq!(OnlineProfile::pending_slash_review(0), None);
        // 质押 bond 未被动（旧模型此处扣 16000 罚 + 1000 申诉质押）：slashable 质押仍为初始 400000
        assert_eq!(OnlineProfile::stash_stake(&machine_stash), 400000 * ONE_DBC);
    })
}

#[test]
fn apply_slash_review_case1_2() {
    after_report_machine_inaccessible1().execute_with(|| {
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();
        let controller = sr25519::Public::from(Sr25519Keyring::Eve).into();

        // [Thread B ③] inaccessible → 0 stake 罚：原断言"罚单 renters 补偿列表"随 slash 归零而移除。
        assert_eq!(
            &OnlineProfile::live_machines(),
            &LiveMachine { offline_machine: vec![machine_id.clone()], ..Default::default() }
        );

        run_to_block(11 + 2 * ONE_DAY);

        assert_ok!(OnlineProfile::controller_report_online(
            RuntimeOrigin::signed(controller),
            machine_id.clone()
        ));

        // 不可达不再罚 stake → 无 PendingSlash
        assert_eq!(OnlineProfile::pending_slash(0), None);
    })
}
