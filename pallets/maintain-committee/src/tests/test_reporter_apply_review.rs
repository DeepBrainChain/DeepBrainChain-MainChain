use super::super::mock::*;
use dbc_support::{
    live_machine::LiveMachine,
    machine_type::MachineStatus,
    verify_slash::{OPPendingSlashInfo, OPPendingSlashReviewInfo, OPSlashReason},
};
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
        let machine_stash: sp_core::sr25519::Public =
            sr25519::Public::from(Sr25519Keyring::Ferdie).into();
        let controller = sr25519::Public::from(Sr25519Keyring::Eve).into();
        let committee = sr25519::Public::from(Sr25519Keyring::One).into();
        let reporter = sr25519::Public::from(Sr25519Keyring::Two).into();

        // let rent_fee = 59890 * 150_000_000 * ONE_DBC / 1000 / 12000;
        let rent_fee = 5240375 * ONE_DBC / 10;
        // 10万为质押，20000为委员会
        assert_eq!(
            Balances::free_balance(machine_stash),
            INIT_BALANCE + rent_fee - 400000 * ONE_DBC - 20000 * ONE_DBC
        );

        assert_eq!(
            &OnlineProfile::live_machines(),
            &LiveMachine { offline_machine: vec![machine_id.clone()], ..Default::default() }
        );

        // Stash apply reonline
        assert_ok!(OnlineProfile::controller_report_online(
            RuntimeOrigin::signed(controller),
            machine_id.clone()
        ));

        let machine_info = OnlineProfile::machines_info(&machine_id).unwrap();
        {
            assert_eq!(machine_info.machine_status, MachineStatus::Rented);
            assert_eq!(
                &OnlineProfile::live_machines(),
                &LiveMachine { rented_machine: vec![machine_id.clone()], ..Default::default() }
            );
            assert_eq!(
                OnlineProfile::pending_slash(0),
                Some(OPPendingSlashInfo {
                    slash_who: machine_stash.clone(),
                    machine_id: machine_id.clone(),
                    slash_time: 24,
                    slash_amount: 16000 * ONE_DBC, // 掉线13个块，惩罚4%: 4000000 * 4% = 16000
                    slash_exec_time: 24 + 2880 * 2,
                    reporter: None, // 这种不奖励验证人
                    renters: vec![reporter],
                    reward_to_committee: Some(vec![committee]),
                    slash_reason: OPSlashReason::RentedInaccessible(11),
                })
            );
        }

        assert_ok!(OnlineProfile::apply_slash_review(RuntimeOrigin::signed(controller), 0, vec![]));
        {
            assert_eq!(
                OnlineProfile::pending_slash_review(0),
                Some(OPPendingSlashReviewInfo {
                    applicant: controller,
                    staked_amount: 1000 * ONE_DBC,
                    apply_time: 24,
                    expire_time: 24 + 2880 * 2,
                    reason: Default::default()
                })
            );
            assert_eq!(
                Balances::free_balance(machine_stash),
                INIT_BALANCE + rent_fee - (400000 + 20000 + 16000 + 1000) * ONE_DBC
            );
        }

        assert_ok!(OnlineProfile::do_cancel_slash(0));
        {
            assert_eq!(OnlineProfile::pending_slash(0), None);
            assert_eq!(OnlineProfile::pending_slash_review(0), None);
            assert_eq!(
                Balances::free_balance(machine_stash),
                INIT_BALANCE + rent_fee - 400000 * ONE_DBC - 20000 * ONE_DBC
            );
        }
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

        // let rent_fee = 59890 * 150_000_000 * ONE_DBC / 1000 / 12000;
        let rent_fee = 5240375 * ONE_DBC / 10;

        // Stash apply reonline
        assert_ok!(OnlineProfile::controller_report_online(
            RuntimeOrigin::signed(controller),
            machine_id.clone()
        ));

        assert_ok!(OnlineProfile::apply_slash_review(RuntimeOrigin::signed(controller), 0, vec![]));

        // TODO: 没有执行取消，则两天后被执行
        run_to_block(25 + 2880 * 2);

        // assert_eq!(<online_profile::PendingSlashReview<TestRuntime>>::contains_key(0), true);
        assert_eq!(OnlineProfile::pending_slash_review(0), None);
        // 机器400000, 委员会质押20000, 申述1000， 罚款16000
        assert_eq!(
            Balances::free_balance(machine_stash),
            INIT_BALANCE + rent_fee
                - 400000 * ONE_DBC
                - 20000 * ONE_DBC
                - 1000 * ONE_DBC
                - 16000 * ONE_DBC
        );
        assert_eq!(OnlineProfile::stash_stake(&machine_stash), 400000 * ONE_DBC);
        assert_eq!(Balances::reserved_balance(&machine_stash), 400000 * ONE_DBC + 20000 * ONE_DBC);
    })
}

#[test]
fn apply_slash_review_case1_2() {
    after_report_machine_inaccessible1().execute_with(|| {
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();
        // let machine_stash: sp_core::sr25519::Public =
        //     sr25519::Public::from(Sr25519Keyring::Ferdie).into();
        let controller = sr25519::Public::from(Sr25519Keyring::Eve).into();
        // let committee = sr25519::Public::from(Sr25519Keyring::One).into();
        let renter = sr25519::Public::from(Sr25519Keyring::Two).into();
        let reporter1 = sr25519::Public::from(Sr25519Keyring::Eve).into();

        assert_eq!(
            &OnlineProfile::live_machines(),
            &LiveMachine { offline_machine: vec![machine_id.clone()], ..Default::default() }
        );

        run_to_block(2880 * 2 + 11);

        // Stash apply reonline
        assert_ok!(OnlineProfile::controller_report_online(
            RuntimeOrigin::signed(controller),
            machine_id.clone()
        ));

        assert_eq!(OnlineProfile::pending_slash(0).unwrap().renters, vec![renter, reporter1]);
    })
}
