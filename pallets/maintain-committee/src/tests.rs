use crate::{mock::*, ReporterStakeInfo};
use frame_support::assert_ok;
use std::convert::TryInto;

#[test]
fn report_machine_fault_works() {
    new_test_with_init_params_ext().execute_with(|| {
        let controller: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();
        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One).into();

        let reporter: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Two).into();
        let reporter_boxpubkey = hex::decode("1e71b5a83ccdeff1592062a1d4da4a272691f08e2024a1ca75a81d534a76210a")
            .unwrap()
            .try_into()
            .unwrap();

        let report_hash: [u8; 16] = hex::decode("986fffc16e63d3f7c43fe1a272ba3ba1").unwrap().try_into().unwrap();

        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();
        let reporter_rand_str = "abcdef".as_bytes().to_vec();
        let committee_rand_str = "fedcba".as_bytes().to_vec();
        let err_reason = "它坏了".as_bytes().to_vec();
        let committee_hash: [u8; 16] = hex::decode("0029f96394d458279bcd0c232365932a").unwrap().try_into().unwrap();

        assert_ok!(MaintainCommittee::report_machine_fault(
            Origin::signed(reporter),
            crate::MachineFaultType::RentedHardwareMalfunction(report_hash, reporter_boxpubkey),
        ));

        // report_machine hardware fault:
        // - Writes:
        // ReporterStake, ReportInfo, LiveReport, ReporterReport
        let report_status = crate::MTReportInfoDetail {
            reporter,
            report_time: 11,
            reporter_stake: 1000 * ONE_DBC, // 15,000,000 / 12,000
            machine_fault_type: crate::MachineFaultType::RentedHardwareMalfunction(report_hash, reporter_boxpubkey),
            ..Default::default()
        };
        assert_eq!(&MaintainCommittee::report_info(0), &report_status);
        assert_eq!(
            &MaintainCommittee::reporter_stake(&reporter),
            &ReporterStakeInfo {
                staked_amount: 20000 * ONE_DBC,
                used_stake: 1000 * ONE_DBC,
                can_claim_reward: 0,
                claimed_reward: 0,
            }
        );
        assert_eq!(
            &MaintainCommittee::live_report(),
            &super::MTLiveReportList { bookable_report: vec![0], ..Default::default() }
        );
        assert_eq!(&MaintainCommittee::reporter_report(&reporter), &super::ReporterRecord { reported_id: vec![0] });

        // 委员会订阅机器故障报告
        assert_ok!(MaintainCommittee::book_fault_order(Origin::signed(committee1), 0));

        // book_fault_order:
        // - Writes:
        // LiveReport, ReportInfo, CommitteeOps, CommitteeOrder
        assert_eq!(
            &MaintainCommittee::live_report(),
            &super::MTLiveReportList { verifying_report: vec![0], ..Default::default() }
        );
        let mut report_info = super::MTReportInfoDetail {
            first_book_time: 11,
            verifying_committee: Some(committee1.clone()),
            booked_committee: vec![committee1.clone()],
            confirm_start: 11 + 360,
            report_status: super::ReportStatus::Verifying,
            ..report_status
        };
        assert_eq!(&MaintainCommittee::report_info(0), &report_info);
        let mut committee_ops = super::MTCommitteeOpsDetail {
            booked_time: 11,
            staked_balance: 1000 * ONE_DBC,
            order_status: super::MTOrderStatus::WaitingEncrypt,
            ..Default::default()
        };
        assert_eq!(&MaintainCommittee::committee_ops(&committee1, 0), &committee_ops);
        assert_eq!(
            &MaintainCommittee::committee_order(&committee1),
            &super::MTCommitteeReportList { booked_report: vec![0], ..Default::default() }
        );

        // 提交加密信息
        let encrypted_err_info: Vec<u8> = hex::decode("01405deeef2a8b0f4a09380d14431dd10fde1ad62b3c27b3fbea4701311d")
            .unwrap()
            .try_into()
            .unwrap();
        assert_ok!(MaintainCommittee::reporter_add_encrypted_error_info(
            Origin::signed(reporter),
            0,
            committee1,
            encrypted_err_info.clone()
        ));

        // add_encrypted_err_info:
        // - Writes:
        // CommitteeOps, ReportInfo

        report_info.get_encrypted_info_committee.push(committee1);
        assert_eq!(&MaintainCommittee::report_info(0), &report_info);
        committee_ops.encrypted_err_info = Some(encrypted_err_info.clone());
        committee_ops.encrypted_time = 11;
        committee_ops.order_status = super::MTOrderStatus::Verifying;

        assert_eq!(&MaintainCommittee::committee_ops(&committee1, 0), &committee_ops);

        // 提交验证Hash
        assert_ok!(MaintainCommittee::submit_confirm_hash(Origin::signed(committee1), 0, committee_hash.clone()));

        // submit_confirm_hash:
        // - Writes:
        // CommitteeOrder, CommitteeOps, ReportInfo, LiveReport

        report_info.verifying_committee = None;
        report_info.hashed_committee.push(committee1);
        report_info.report_status = crate::ReportStatus::WaitingBook;
        assert_eq!(&MaintainCommittee::report_info(0), &report_info);
        assert_eq!(
            &MaintainCommittee::live_report(),
            &super::MTLiveReportList { bookable_report: vec![0], ..Default::default() }
        );
        committee_ops.confirm_hash = committee_hash;
        committee_ops.order_status = super::MTOrderStatus::WaitingRaw;
        committee_ops.hash_time = 11;
        assert_eq!(&MaintainCommittee::committee_ops(&committee1, 0), &committee_ops);
        assert_eq!(
            &MaintainCommittee::committee_order(&committee1),
            &super::MTCommitteeReportList { hashed_report: vec![0], ..Default::default() }
        );

        // 3个小时之后才能提交：
        run_to_block(360 + 13);

        report_info.report_status = crate::ReportStatus::SubmittingRaw;
        assert_eq!(&MaintainCommittee::report_info(0), &report_info);
        assert_eq!(
            &MaintainCommittee::live_report(),
            &super::MTLiveReportList { waiting_raw_report: vec![0], ..Default::default() }
        );

        // submit_confirm_raw:
        // - Writes:
        // ReportInfo, CommitteeOps
        let extra_err_info = Vec::new();
        assert_ok!(MaintainCommittee::submit_confirm_raw(
            Origin::signed(committee1),
            0,
            machine_id.clone(),
            reporter_rand_str,
            committee_rand_str,
            err_reason.clone(),
            extra_err_info,
            true
        ));

        report_info.confirmed_committee = vec![committee1.clone()];
        report_info.support_committee = vec![committee1.clone()];
        report_info.machine_id = machine_id.clone();
        report_info.err_info = err_reason;
        assert_eq!(&MaintainCommittee::report_info(0), &report_info);

        committee_ops.confirm_time = 374;
        committee_ops.confirm_result = true;
        committee_ops.order_status = super::MTOrderStatus::Finished;

        assert_eq!(&MaintainCommittee::committee_ops(&committee1, 0), &committee_ops);

        run_to_block(360 + 14);

        // heart_beat will change LiveReport,
        // report_info.report_status = super::ReportStatus::CommitteeConfirmed; // TODO: should be cleaned
        assert_eq!(&MaintainCommittee::report_info(0), &report_info);
        assert_eq!(
            &MaintainCommittee::live_report(),
            &super::MTLiveReportList { waiting_rechecked_report: vec![0], ..Default::default() }
        );

        // mt_machine_offline -> machine_offline
        // - Writes:
        // MachineInfo, LiveMachine, current_era_stash_snap, next_era_stash_snap, current_era_machine_snap, next_era_machine_snap
        // SysInfo, SatshMachine, PosGPUInfo

        // TODO: check status

        assert_eq!(
            &MaintainCommittee::live_report(),
            &super::MTLiveReportList { waiting_rechecked_report: vec![0], ..Default::default() }
        );

        run_to_block(2880 + 400);

        // 报告人上线机器
        assert_ok!(OnlineProfile::controller_report_online(Origin::signed(controller), machine_id.clone()));

        // TODO: Slash
    })
}

#[test]
fn report_machine_offline_works() {
    new_test_with_init_params_ext().execute_with(|| {
        let committee: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One).into();
        let reporter: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Two).into();
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();

        assert_ok!(MaintainCommittee::report_machine_fault(
            Origin::signed(reporter),
            crate::MachineFaultType::RentedInaccessible(machine_id),
        ));

        // 委员会订阅机器故障报告
        assert_ok!(MaintainCommittee::book_fault_order(Origin::signed(committee), 0));

        // book_fault_order:
        // - Writes:
        // LiveReport, ReportInfo, CommitteeOps, CommitteeOrder
        assert_eq!(
            &MaintainCommittee::live_report(),
            &super::MTLiveReportList { bookable_report: vec![0], ..Default::default() }
        );

        // 首先提交Hash: 内容为 订单ID + 验证人自己的随机数 + 机器是否有问题
        // 提交验证Hash: TODO: 更改Hash的内容
        // hash(0abcd1) => 0x73124a023f585b4018b9ed3593c7470a
        let offline_committee_hash: [u8; 16] =
            hex::decode("73124a023f585b4018b9ed3593c7470a").unwrap().try_into().unwrap();
        assert_ok!(MaintainCommittee::submit_confirm_hash(
            Origin::signed(committee),
            0,
            offline_committee_hash.clone()
        ));

        run_to_block(21);
        assert_ok!(MaintainCommittee::submit_offline_raw(
            Origin::signed(committee),
            0,
            "abcd".as_bytes().to_vec(),
            true
        ));
        run_to_block(22);
    })
}

// 1. case1. 第一个报告人没有在半个小时内提交错误信息, ..第二个， ..第三个
// 2. case2. 第一个委员会没有在抢单1小时内提交错误Hash, ..第二个，..第三个
// 3. case3. 该订单提前结束，且结束时，距离委员会抢单还没到一个小时，最后一个委员会是第三个抢单委员会，
// ..是第二个委员会， ..是第三个委员会

// 1. case1. 第一个报告人没有在半个小时内提交错误信息, ..第二个， ..第三个
#[test]
fn test_heart_beat1() {
    new_test_with_init_params_ext().execute_with(|| {
        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One).into();

        let reporter: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();
        let reporter_boxpubkey = hex::decode("1e71b5a83ccdeff1592062a1d4da4a272691f08e2024a1ca75a81d534a76210a")
            .unwrap()
            .try_into()
            .unwrap();
        let report_hash: [u8; 16] = hex::decode("986fffc16e63d3f7c43fe1a272ba3ba1").unwrap().try_into().unwrap();

        assert_ok!(MaintainCommittee::report_machine_fault(
            Origin::signed(reporter),
            crate::MachineFaultType::RentedHardwareMalfunction(report_hash, reporter_boxpubkey),
        ));

        // report_machine hardware fault:
        // - Writes:
        // ReporterStake, ReportInfo, LiveReport, ReporterReport

        // 委员会订阅机器故障报告
        assert_ok!(MaintainCommittee::book_fault_order(Origin::signed(committee1), 0));

        // book_fault_order:
        // - Writes:
        // LiveReport, ReportInfo, CommitteeOps, CommitteeOrder

        // 预订时间为11
        run_to_block(11 + 61);

        // hertbeat will run, and report info will be deleted
        // - Writes:
        // LiveReport, CommitteeReport, CommitteeOps, ReportInfo
        assert_eq!(MaintainCommittee::live_report(), super::MTLiveReportList { ..Default::default() });
        assert_eq!(
            MaintainCommittee::committee_order(&committee1),
            super::MTCommitteeReportList { ..Default::default() }
        );
        assert_eq!(
            MaintainCommittee::committee_ops(&committee1, 0),
            super::MTCommitteeOpsDetail { ..Default::default() }
        );
        assert_eq!(MaintainCommittee::report_info(0), super::MTReportInfoDetail { ..Default::default() });
        assert_eq!(
            MaintainCommittee::pending_slash(0),
            super::MTPendingSlashInfo {
                slash_who: reporter.clone(),
                slash_time: 11 + 60,
                slash_amount: 1000 * ONE_DBC,
                slash_exec_time: 11 + 60 + 2880 * 2,
                reward_to: vec![],
                slash_reason: super::MTReporterSlashReason::NotSubmitEncryptedInfo
            }
        );

        // 运行到某个时间
    })
}

// 2. case2. 第一个委员会没有在抢单1小时内提交错误Hash, ..第二个，..第三个
#[test]
fn test_heart_beat2() {
    new_test_with_init_params_ext().execute_with(|| {
        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One).into();

        let reporter: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();
        let reporter_boxpubkey = hex::decode("1e71b5a83ccdeff1592062a1d4da4a272691f08e2024a1ca75a81d534a76210a")
            .unwrap()
            .try_into()
            .unwrap();
        let report_hash: [u8; 16] = hex::decode("986fffc16e63d3f7c43fe1a272ba3ba1").unwrap().try_into().unwrap();

        assert_ok!(MaintainCommittee::report_machine_fault(
            Origin::signed(reporter),
            crate::MachineFaultType::RentedHardwareMalfunction(report_hash, reporter_boxpubkey),
        ));

        // report_machine hardware fault:
        // - Writes:
        // ReporterStake, ReportInfo, LiveReport, ReporterReport

        // 委员会订阅机器故障报告
        assert_ok!(MaintainCommittee::book_fault_order(Origin::signed(committee1), 0));

        // book_fault_order:
        // - Writes:
        // LiveReport, ReportInfo, CommitteeOps, CommitteeOrder

        // 提交加密信息
        let encrypted_err_info: Vec<u8> = hex::decode("01405deeef2a8b0f4a09380d14431dd10fde1ad62b3c27b3fbea4701311d")
            .unwrap()
            .try_into()
            .unwrap();
        assert_ok!(MaintainCommittee::reporter_add_encrypted_error_info(
            Origin::signed(reporter),
            0,
            committee1,
            encrypted_err_info.clone()
        ));

        // add_encrypted_err_info:
        // - Writes:
        // CommitteeOps, ReportInfo

        // 预订时间为11
        run_to_block(11 + 121);

        // hertbeat will run, and report info will be deleted
        // - Writes:
        // LiveReport, CommitteeReport, CommitteeOps, ReportInfo
        assert_eq!(
            MaintainCommittee::live_report(),
            super::MTLiveReportList { bookable_report: vec![0], ..Default::default() }
        );
        assert_eq!(
            MaintainCommittee::committee_order(&committee1),
            super::MTCommitteeReportList { ..Default::default() }
        );
        assert_eq!(
            MaintainCommittee::committee_ops(&committee1, 0),
            super::MTCommitteeOpsDetail { ..Default::default() }
        );
        assert_eq!(
            MaintainCommittee::report_info(0),
            super::MTReportInfoDetail {
                reporter: reporter.clone(),
                report_time: 11,
                reporter_stake: 1000 * ONE_DBC,
                report_status: super::ReportStatus::Reported,
                machine_fault_type: super::MachineFaultType::RentedHardwareMalfunction(report_hash, reporter_boxpubkey),
                ..Default::default()
            }
        );
        assert_eq!(
            Committee::pending_slash(0),
            committee::CMPendingSlashInfo {
                slash_who: committee1.clone(),
                slash_time: 11 + 120,
                slash_amount: 1000 * ONE_DBC,
                slash_exec_time: 11 + 120 + 2880 * 2,
                reward_to: vec![],
                slash_reason: committee::CMSlashReason::MCNotSubmitHash,
            }
        );
    })
}

// 3. case3. 该订单提前结束，且结束时，距离委员会抢单还没到一个小时，最后一个委员会是第二个抢单委员会，
#[test]
fn test_heart_beat3() {
    new_test_with_init_params_ext().execute_with(|| {
        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One).into();
        let committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Two).into();

        let reporter: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();
        let reporter_boxpubkey = hex::decode("1e71b5a83ccdeff1592062a1d4da4a272691f08e2024a1ca75a81d534a76210a")
            .unwrap()
            .try_into()
            .unwrap();
        let report_hash: [u8; 16] = hex::decode("986fffc16e63d3f7c43fe1a272ba3ba1").unwrap().try_into().unwrap();

        assert_ok!(MaintainCommittee::report_machine_fault(
            Origin::signed(reporter),
            crate::MachineFaultType::RentedHardwareMalfunction(report_hash, reporter_boxpubkey),
        ));

        // report_machine hardware fault:
        // - Writes:
        // ReporterStake, ReportInfo, LiveReport, ReporterReport

        // 委员会订阅机器故障报告
        assert_ok!(MaintainCommittee::book_fault_order(Origin::signed(committee1), 0));

        // book_fault_order:
        // - Writes:
        // LiveReport, ReportInfo, CommitteeOps, CommitteeOrder

        // 提交加密信息
        let encrypted_err_info: Vec<u8> = hex::decode("01405deeef2a8b0f4a09380d14431dd10fde1ad62b3c27b3fbea4701311d")
            .unwrap()
            .try_into()
            .unwrap();
        assert_ok!(MaintainCommittee::reporter_add_encrypted_error_info(
            Origin::signed(reporter),
            0,
            committee1,
            encrypted_err_info.clone()
        ));

        // add_encrypted_err_info:
        // - Writes:
        // CommitteeOps, ReportInfo

        // 预订时间为11
        run_to_block(11 + 60);

        // 提交验证Hash
        let committee_hash: [u8; 16] = hex::decode("0029f96394d458279bcd0c232365932a").unwrap().try_into().unwrap();
        assert_ok!(MaintainCommittee::submit_confirm_hash(Origin::signed(committee1), 0, committee_hash.clone()));

        // 第二个委员会去预订: 第一次预订时间为11， 则开始提交原始值时间为11 + 3*120
        run_to_block(3 * 120);
        // 委员会订阅机器故障报告
        assert_ok!(MaintainCommittee::book_fault_order(Origin::signed(committee2), 0));
        // 报告人给第二个委员会提供加密信息
        assert_ok!(MaintainCommittee::reporter_add_encrypted_error_info(
            Origin::signed(reporter),
            0,
            committee2,
            encrypted_err_info.clone()
        ));
        // 第二个委员会没来得及提交原始值

        run_to_block(11 + 3 * 120 + 1);

        // hertbeat will run, and report info will be deleted
        // - Writes:
        // LiveReport, CommitteeReport, CommitteeOps, ReportInfo
        assert_eq!(
            MaintainCommittee::live_report(),
            super::MTLiveReportList { waiting_raw_report: vec![0], ..Default::default() }
        );
        assert_eq!(
            MaintainCommittee::committee_order(&committee2),
            super::MTCommitteeReportList { ..Default::default() }
        );
        assert_eq!(
            MaintainCommittee::committee_ops(&committee2, 0),
            super::MTCommitteeOpsDetail { ..Default::default() }
        );
        assert_eq!(
            MaintainCommittee::report_info(0),
            super::MTReportInfoDetail {
                reporter: reporter.clone(),
                report_time: 11,
                first_book_time: 11,
                booked_committee: vec![committee1],
                get_encrypted_info_committee: vec![committee1],
                hashed_committee: vec![committee1],
                confirm_start: 371,
                reporter_stake: 1000 * ONE_DBC,
                report_status: super::ReportStatus::SubmittingRaw,
                machine_fault_type: super::MachineFaultType::RentedHardwareMalfunction(report_hash, reporter_boxpubkey),
                ..Default::default()
            }
        );
    })
}

#[test]
fn test_heart_beat4() {}

#[test]
fn test_heart_beat5() {}

#[test]
fn test_heart_beat6() {}

#[test]
fn test_heart_beat7() {}

#[test]
fn test_offline_case1() {}

#[test]
fn test_offline_case2() {}

#[test]
fn test_offline_case3() {}

#[test]
fn report_machine_unrentable_works() {}

// 控制账户报告机器下线
#[test]
fn controller_report_online_machine_offline_should_work() {
    new_test_with_init_params_ext().execute_with(|| {})
}

// 检查各种情况下，live_report状态正确变化
#[test]
fn live_report_status_change_works() {}
