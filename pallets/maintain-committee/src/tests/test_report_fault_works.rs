use super::super::{mock::*, ReporterStakeInfo};
use frame_support::assert_ok;
use std::convert::TryInto;

// 报告机器被租用，但是无法访问
#[test]
fn report_machine_inaccessible_works() {
    new_test_with_init_params_ext().execute_with(|| {
        let committee: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One).into();
        let reporter: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Two).into();
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();

        // 记录：ReportInfo, LiveReport, ReporterReport 并支付处理所需的金额
        assert_ok!(MaintainCommittee::report_machine_fault(
            Origin::signed(reporter),
            crate::MachineFaultType::RentedInaccessible(machine_id.clone()),
        ));

        // 判断举报之后的状态
        {
            assert_eq!(
                &MaintainCommittee::live_report(),
                &crate::MTLiveReportList { bookable_report: vec![0], ..Default::default() }
            );
            assert_eq!(
                &MaintainCommittee::report_info(0),
                &crate::MTReportInfoDetail {
                    reporter,
                    report_time: 11,
                    reporter_stake: 1000 * ONE_DBC,
                    machine_id: machine_id.clone(),
                    machine_fault_type: crate::MachineFaultType::RentedInaccessible(machine_id),
                    report_status: crate::ReportStatus::Reported,

                    ..Default::default()
                }
            );
            assert_eq!(
                &MaintainCommittee::reporter_report(&reporter),
                &crate::ReporterReportList { processing_report: vec![0], ..Default::default() }
            );
        }

        // 委员会订阅机器故障报告
        assert_ok!(MaintainCommittee::committee_book_report(Origin::signed(committee), 0));

        // 检查订阅之后的状态
        {
            // do_report_machine_fault:
            // - Writes:
            // LiveReport, ReportInfo, CommitteeOps, CommitteeOrder
            assert_eq!(
                &MaintainCommittee::live_report(),
                &crate::MTLiveReportList { bookable_report: vec![0], ..Default::default() }
            );
            // TODO: add other check
        }

        // 首先提交Hash: 内容为 订单ID + 验证人自己的随机数 + 机器是否有问题
        // hash(0abcd1) => 0x73124a023f585b4018b9ed3593c7470a
        let offline_committee_hash: [u8; 16] =
            hex::decode("73124a023f585b4018b9ed3593c7470a").unwrap().try_into().unwrap();
        assert_ok!(MaintainCommittee::committee_submit_verify_hash(
            Origin::signed(committee),
            0,
            offline_committee_hash.clone()
        ));

        run_to_block(21);
        assert_ok!(MaintainCommittee::committee_submit_offline_raw(
            Origin::signed(committee),
            0,
            "abcd".as_bytes().to_vec(),
            true
        ));
        run_to_block(22);
    })
}

#[test]
fn report_machine_fault_works() {
    new_test_with_init_params_ext().execute_with(|| {
        let controller: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();
        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One).into();

        // 报告人
        let reporter: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Two).into();
        // 报告人解密pubkey
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
            &crate::MTLiveReportList { bookable_report: vec![0], ..Default::default() }
        );
        assert_eq!(
            &MaintainCommittee::reporter_report(&reporter),
            &crate::ReporterReportList { processing_report: vec![0], ..Default::default() }
        );

        // 委员会订阅机器故障报告
        assert_ok!(MaintainCommittee::committee_book_report(Origin::signed(committee1), 0));

        // book_fault_order:
        // - Writes:
        // LiveReport, ReportInfo, CommitteeOps, CommitteeOrder
        assert_eq!(
            &MaintainCommittee::live_report(),
            &crate::MTLiveReportList { verifying_report: vec![0], ..Default::default() }
        );
        let mut report_info = crate::MTReportInfoDetail {
            first_book_time: 11,
            verifying_committee: Some(committee1.clone()),
            booked_committee: vec![committee1.clone()],
            confirm_start: 11 + 360,
            report_status: crate::ReportStatus::Verifying,
            ..report_status
        };
        assert_eq!(&MaintainCommittee::report_info(0), &report_info);
        let mut committee_ops = crate::MTCommitteeOpsDetail {
            booked_time: 11,
            staked_balance: 1000 * ONE_DBC,
            order_status: crate::MTOrderStatus::WaitingEncrypt,
            ..Default::default()
        };
        assert_eq!(&MaintainCommittee::committee_ops(&committee1, 0), &committee_ops);
        assert_eq!(
            &MaintainCommittee::committee_order(&committee1),
            &crate::MTCommitteeOrderList { booked_report: vec![0], ..Default::default() }
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
        committee_ops.order_status = crate::MTOrderStatus::Verifying;

        assert_eq!(&MaintainCommittee::committee_ops(&committee1, 0), &committee_ops);

        // 提交验证Hash
        assert_ok!(MaintainCommittee::committee_submit_verify_hash(
            Origin::signed(committee1),
            0,
            committee_hash.clone()
        ));

        // submit_confirm_hash:
        // - Writes:
        // CommitteeOrder, CommitteeOps, ReportInfo, LiveReport

        report_info.verifying_committee = None;
        report_info.hashed_committee.push(committee1);
        report_info.report_status = crate::ReportStatus::WaitingBook;
        assert_eq!(&MaintainCommittee::report_info(0), &report_info);
        assert_eq!(
            &MaintainCommittee::live_report(),
            &crate::MTLiveReportList { bookable_report: vec![0], ..Default::default() }
        );
        committee_ops.confirm_hash = committee_hash;
        committee_ops.order_status = crate::MTOrderStatus::WaitingRaw;
        committee_ops.hash_time = 11;
        assert_eq!(&MaintainCommittee::committee_ops(&committee1, 0), &committee_ops);
        assert_eq!(
            &MaintainCommittee::committee_order(&committee1),
            &crate::MTCommitteeOrderList { hashed_report: vec![0], ..Default::default() }
        );

        // 3个小时之后才能提交：
        run_to_block(360 + 13);

        report_info.report_status = crate::ReportStatus::SubmittingRaw;
        assert_eq!(&MaintainCommittee::report_info(0), &report_info);
        assert_eq!(
            &MaintainCommittee::live_report(),
            &crate::MTLiveReportList { waiting_raw_report: vec![0], ..Default::default() }
        );

        // submit_confirm_raw:
        // - Writes:
        // ReportInfo, CommitteeOps
        let extra_err_info = Vec::new();
        assert_ok!(MaintainCommittee::committee_submit_verify_raw(
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
        committee_ops.order_status = crate::MTOrderStatus::Finished;

        assert_eq!(&MaintainCommittee::committee_ops(&committee1, 0), &committee_ops);

        assert_eq!(
            &MaintainCommittee::live_report(),
            &crate::MTLiveReportList { waiting_raw_report: vec![0], ..Default::default() }
        );

        assert!(match MaintainCommittee::summary_report(0) {
            crate::ReportConfirmStatus::Confirmed(..) => true,
            _ => false,
        });

        // assert_eq!(&super::ReportConfirmStatus::Confirmed(_, _, _), MaintainCommittee::summary_report(0));

        run_to_block(360 + 14);

        // summary_fault_case -> summary_waiting_raw -> Confirmed -> mt_machine_offline
        // - Writes:
        // committee_stake; committee_order; LiveReport;
        // report_info.report_status = super::ReportStatus::CommitteeConfirmed;
        assert_eq!(Committee::committee_stake(committee1).used_stake, 0);
        assert_eq!(
            MaintainCommittee::committee_order(committee1),
            crate::MTCommitteeOrderList { finished_report: vec![0], ..Default::default() }
        );
        // assert_eq!(&MachineCommittee::report_info(0), &super::MTReportInfoDetail { ..Default::default() });
        // assert_eq!(&MaintainCommittee::report_info(0), &report_info);
        assert_eq!(
            &MaintainCommittee::live_report(),
            &crate::MTLiveReportList { finished_report: vec![0], ..Default::default() }
        );

        // mt_machine_offline -> machine_offline
        // - Writes:
        // MachineInfo, LiveMachine, current_era_stash_snap, next_era_stash_snap, current_era_machine_snap, next_era_machine_snap
        // SysInfo, SatshMachine, PosGPUInfo

        assert_eq!(
            &MaintainCommittee::live_report(),
            &crate::MTLiveReportList { finished_report: vec![0], ..Default::default() }
        );

        run_to_block(2880 + 400);

        // 报告人上线机器
        assert_ok!(OnlineProfile::controller_report_online(Origin::signed(controller), machine_id.clone()));
    })
}
