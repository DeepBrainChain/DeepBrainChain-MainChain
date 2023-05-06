use super::super::{mock::*, ReporterStakeInfo};
use frame_support::assert_ok;
use once_cell::sync::Lazy;
use std::convert::TryInto;

const controller: Lazy<sp_core::sr25519::Public> =
    Lazy::new(|| sr25519::Public::from(Sr25519Keyring::Eve));

const committee1: Lazy<sp_core::sr25519::Public> =
    Lazy::new(|| sr25519::Public::from(Sr25519Keyring::One));
const committee2: Lazy<sp_core::sr25519::Public> =
    Lazy::new(|| sr25519::Public::from(Sr25519Keyring::Two));
const committee3: Lazy<sp_core::sr25519::Public> =
    Lazy::new(|| sr25519::Public::from(Sr25519Keyring::Ferdie));

const reporter: Lazy<sp_core::sr25519::Public> = committee2;

// case1: 1个委员会预订并处理
// case2: 3个委员会预订，并正常处理，通过报告
// case3: 3个委员会预订，否定报告
// case4: 1个委员会预订，都提交Hash，没有提交原始值，将调用重新派单

// 报告其他类型的错误
#[test]
fn report_machine_fault_works_case1() {
    new_test_with_init_params_ext().execute_with(|| {
        let reporter_boxpubkey =
            hex::decode("1e71b5a83ccdeff1592062a1d4da4a272691f08e2024a1ca75a81d534a76210a")
                .unwrap()
                .try_into()
                .unwrap();
        let report_hash: [u8; 16] =
            hex::decode("2611557f5306f050019eeb27648c5494").unwrap().try_into().unwrap();

        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();
        let reporter_rand_str = "abcdef".as_bytes().to_vec();
        let committee_rand_str = "abc1".as_bytes().to_vec();
        let err_reason = "补充信息，可留空".as_bytes().to_vec();
        let committee_hash: [u8; 16] =
            hex::decode("7980cfd18a2e6cb338f4924ae0fff495").unwrap().try_into().unwrap();

        assert_ok!(MaintainCommittee::report_machine_fault(
            RuntimeOrigin::signed(*reporter),
            crate::MachineFaultType::RentedHardwareMalfunction(report_hash, reporter_boxpubkey),
        ));

        // report_machine hardware fault:
        // - Writes:
        // ReporterStake, ReportInfo, LiveReport, ReporterReport
        let report_status = crate::MTReportInfoDetail {
            reporter: *reporter,
            report_time: 11,
            reporter_stake: 1000 * ONE_DBC, // 15,000,000 / 12,000
            machine_fault_type: crate::MachineFaultType::RentedHardwareMalfunction(
                report_hash,
                reporter_boxpubkey,
            ),
            first_book_time: todo!(),
            machine_id,
            rent_order_id: todo!(),
            err_info: todo!(),
            verifying_committee: todo!(),
            booked_committee: todo!(),
            get_encrypted_info_committee: todo!(),
            hashed_committee: todo!(),
            confirm_start: todo!(),
            confirmed_committee: todo!(),
            support_committee: todo!(),
            against_committee: todo!(),
            report_status: todo!(),
        };
        {
            assert_eq!(MaintainCommittee::report_info(0), Some(report_status));
            assert_eq!(
                &MaintainCommittee::reporter_stake(&*reporter),
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
                &MaintainCommittee::reporter_report(&*reporter),
                &crate::ReporterReportList { processing_report: vec![0], ..Default::default() }
            );
        }

        // 委员会订阅机器故障报告
        assert_ok!(MaintainCommittee::committee_book_report(RuntimeOrigin::signed(*committee1), 0));

        let mut report_info = crate::MTReportInfoDetail {
            first_book_time: 11,
            verifying_committee: Some(committee1.clone()),
            booked_committee: vec![committee1.clone()],
            confirm_start: 11 + 360,
            report_status: crate::ReportStatus::Verifying,
            ..report_status
        };
        let mut committee_ops = crate::MTCommitteeOpsDetail {
            booked_time: 11,
            staked_balance: 1000 * ONE_DBC,
            order_status: crate::MTOrderStatus::WaitingEncrypt,
            ..Default::default()
        };

        {
            // book_fault_order:
            // - Writes:
            // LiveReport, ReportInfo, CommitteeOps, CommitteeOrder
            assert_eq!(
                &MaintainCommittee::live_report(),
                &crate::MTLiveReportList { verifying_report: vec![0], ..Default::default() }
            );
            assert_eq!(MaintainCommittee::report_info(0), Some(report_info));
            assert_eq!(&MaintainCommittee::committee_ops(&*committee1, 0), &committee_ops);
            assert_eq!(
                &MaintainCommittee::committee_order(&*committee1),
                &crate::MTCommitteeOrderList { booked_report: vec![0], ..Default::default() }
            );
        }

        // 提交加密信息
        let encrypted_err_info: Vec<u8> =
            hex::decode("01405deeef2a8b0f4a09380d14431dd10fde1ad62b3c27b3fbea4701311d")
                .unwrap()
                .try_into()
                .unwrap();
        {
            assert_ok!(MaintainCommittee::reporter_add_encrypted_error_info(
                RuntimeOrigin::signed(*reporter),
                0,
                *committee1,
                encrypted_err_info.clone()
            ));

            // add_encrypted_err_info:
            // - Writes:
            // CommitteeOps, ReportInfo

            report_info.get_encrypted_info_committee.push(*committee1);
            assert_eq!(MaintainCommittee::report_info(0), Some(report_info));
            committee_ops.encrypted_err_info = Some(encrypted_err_info.clone());
            committee_ops.encrypted_time = 11;
            committee_ops.order_status = crate::MTOrderStatus::Verifying;

            assert_eq!(&MaintainCommittee::committee_ops(&*committee1, 0), &committee_ops);
        }

        // 提交验证Hash
        assert_ok!(MaintainCommittee::committee_submit_verify_hash(
            RuntimeOrigin::signed(*committee1),
            0,
            committee_hash.clone()
        ));

        {
            // submit_confirm_hash:
            // - Writes:
            // CommitteeOrder, CommitteeOps, ReportInfo, LiveReport
            report_info.verifying_committee = None;
            report_info.hashed_committee.push(*committee1);
            report_info.report_status = crate::ReportStatus::WaitingBook;
            assert_eq!(MaintainCommittee::report_info(0), Some(report_info));
            assert_eq!(
                &MaintainCommittee::live_report(),
                &crate::MTLiveReportList { bookable_report: vec![0], ..Default::default() }
            );
            committee_ops.confirm_hash = committee_hash;
            committee_ops.order_status = crate::MTOrderStatus::WaitingRaw;
            committee_ops.hash_time = 11;
            assert_eq!(&MaintainCommittee::committee_ops(&*committee1, 0), &committee_ops);
            assert_eq!(
                &MaintainCommittee::committee_order(&*committee1),
                &crate::MTCommitteeOrderList { hashed_report: vec![0], ..Default::default() }
            );
        }

        // 3个小时之后才能提交：
        run_to_block(360 + 13);
        {
            report_info.report_status = crate::ReportStatus::SubmittingRaw;
            assert_eq!(MaintainCommittee::report_info(0), Some(report_info));
            assert_eq!(
                &MaintainCommittee::live_report(),
                &crate::MTLiveReportList { waiting_raw_report: vec![0], ..Default::default() }
            );
        }

        // submit_confirm_raw:
        // - Writes:
        // ReportInfo, CommitteeOps
        let extra_err_info = Vec::new();
        assert_ok!(MaintainCommittee::committee_submit_verify_raw(
            RuntimeOrigin::signed(*committee1),
            0,
            machine_id.clone(),
            0,
            reporter_rand_str,
            committee_rand_str,
            err_reason.clone(),
            extra_err_info,
            true
        ));

        {
            report_info.confirmed_committee = vec![committee1.clone()];
            report_info.support_committee = vec![committee1.clone()];
            report_info.machine_id = machine_id.clone();
            report_info.err_info = err_reason;
            assert_eq!(MaintainCommittee::report_info(0), Some(report_info));

            committee_ops.confirm_time = 374;
            committee_ops.confirm_result = true;
            committee_ops.order_status = crate::MTOrderStatus::Finished;

            assert_eq!(&MaintainCommittee::committee_ops(&*committee1, 0), &committee_ops);

            assert_eq!(
                &MaintainCommittee::live_report(),
                &crate::MTLiveReportList { waiting_raw_report: vec![0], ..Default::default() }
            );

            assert!(match report_info.summary() {
                crate::ReportConfirmStatus::Confirmed(..) => true,
                _ => false,
            });
        }

        // assert_eq!(&super::ReportConfirmStatus::Confirmed(_, _, _),
        // MaintainCommittee::summary_report(0));

        run_to_block(360 + 14);

        {
            // summary_fault_case -> summary_waiting_raw -> Confirmed -> mt_machine_offline
            // - Writes:
            // committee_stake; committee_order; LiveReport;
            // report_info.report_status = super::ReportStatus::CommitteeConfirmed;
            assert_eq!(Committee::committee_stake(*committee1).used_stake, 1000 * ONE_DBC);
            assert_eq!(
                MaintainCommittee::committee_order(*committee1),
                crate::MTCommitteeOrderList { finished_report: vec![0], ..Default::default() }
            );
            // assert_eq!(&MachineCommittee::report_info(0), &super::MTReportInfoDetail {
            // ..Default::default() }); assert_eq!(&MaintainCommittee::report_info(0),
            // &report_info);
            assert_eq!(
                &MaintainCommittee::live_report(),
                &crate::MTLiveReportList { finished_report: vec![0], ..Default::default() }
            );

            // mt_machine_offline -> machine_offline
            // - Writes:
            // MachineInfo, LiveMachine, current_era_stash_snap, next_era_stash_snap,
            // current_era_machine_snap, next_era_machine_snap SysInfo, SatshMachine,
            // PosGPUInfo

            assert_eq!(
                &MaintainCommittee::live_report(),
                &crate::MTLiveReportList { finished_report: vec![0], ..Default::default() }
            );
            // NOTE: 没有任何反对的成功举报，同样需要记录
            assert_eq!(MaintainCommittee::unhandled_report_result(374 + 2880 * 2), vec![0]);
        }

        run_to_block(2880 * 2 + 374);
        {
            assert_eq!(
                MaintainCommittee::reporter_stake(&*reporter),
                crate::ReporterStakeInfo { staked_amount: 20000 * ONE_DBC, ..Default::default() }
            );
            assert_eq!(Committee::committee_stake(*committee1).used_stake, 0);
            assert_eq!(Committee::committee_stake(*committee1).staked_amount, 20000 * ONE_DBC);
        }

        // 报告人上线机器
        assert_ok!(OnlineProfile::controller_report_online(
            RuntimeOrigin::signed(*controller),
            machine_id.clone()
        ));
    })
}

#[test]
fn report_machine_fault_works_case2() {
    new_test_with_init_params_ext().execute_with(|| {
        let reporter_boxpubkey =
            hex::decode("1e71b5a83ccdeff1592062a1d4da4a272691f08e2024a1ca75a81d534a76210a")
                .unwrap()
                .try_into()
                .unwrap();

        let report_hash: [u8; 16] =
            hex::decode("2611557f5306f050019eeb27648c5494").unwrap().try_into().unwrap();

        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();
        let reporter_rand_str = "abcdef".as_bytes().to_vec();

        let committee1_rand_str = "abc1".as_bytes().to_vec();
        let committee2_rand_str = "abc2".as_bytes().to_vec();
        let committee3_rand_str = "abc3".as_bytes().to_vec();
        let err_reason = "补充信息，可留空".as_bytes().to_vec();

        let committee1_hash: [u8; 16] =
            hex::decode("7980cfd18a2e6cb338f4924ae0fff495").unwrap().try_into().unwrap();
        let committee2_hash: [u8; 16] =
            hex::decode("b4b78fecc59dfab6b7b75614ea12e39b").unwrap().try_into().unwrap();
        let committee3_hash: [u8; 16] =
            hex::decode("eb2f9710489d601925b1c2b885578264").unwrap().try_into().unwrap();

        assert_ok!(MaintainCommittee::report_machine_fault(
            RuntimeOrigin::signed(*reporter),
            crate::MachineFaultType::RentedHardwareMalfunction(report_hash, reporter_boxpubkey),
        ));

        // report_machine hardware fault:
        // - Writes:
        // ReporterStake, ReportInfo, LiveReport, ReporterReport
        let report_status = crate::MTReportInfoDetail {
            reporter: *reporter,
            report_time: 11,
            reporter_stake: 1000 * ONE_DBC, // 15,000,000 / 12,000
            machine_fault_type: crate::MachineFaultType::RentedHardwareMalfunction(
                report_hash,
                reporter_boxpubkey,
            ),
            first_book_time: todo!(),
            machine_id,
            rent_order_id: todo!(),
            err_info: todo!(),
            verifying_committee: todo!(),
            booked_committee: todo!(),
            get_encrypted_info_committee: todo!(),
            hashed_committee: todo!(),
            confirm_start: todo!(),
            confirmed_committee: todo!(),
            support_committee: todo!(),
            against_committee: todo!(),
            report_status: todo!(),
        };
        {
            assert_eq!(MaintainCommittee::report_info(0), Some(report_status));
            assert_eq!(
                &MaintainCommittee::reporter_stake(&*reporter),
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
                &MaintainCommittee::reporter_report(&*reporter),
                &crate::ReporterReportList { processing_report: vec![0], ..Default::default() }
            );
        }

        // 委员会订阅机器故障报告
        assert_ok!(MaintainCommittee::committee_book_report(RuntimeOrigin::signed(*committee1), 0));

        let mut report_info = crate::MTReportInfoDetail {
            first_book_time: 11,
            verifying_committee: Some(committee1.clone()),
            booked_committee: vec![committee1.clone()],
            confirm_start: 11 + 360,
            report_status: crate::ReportStatus::Verifying,
            ..report_status
        };
        let mut committee_ops = crate::MTCommitteeOpsDetail {
            booked_time: 11,
            staked_balance: 1000 * ONE_DBC,
            order_status: crate::MTOrderStatus::WaitingEncrypt,
            ..Default::default()
        };

        {
            // book_fault_order:
            // - Writes:
            // LiveReport, ReportInfo, CommitteeOps, CommitteeOrder
            assert_eq!(
                &MaintainCommittee::live_report(),
                &crate::MTLiveReportList { verifying_report: vec![0], ..Default::default() }
            );
            assert_eq!(MaintainCommittee::report_info(0), Some(report_info));
            assert_eq!(&MaintainCommittee::committee_ops(&*committee1, 0), &committee_ops);
            assert_eq!(
                &MaintainCommittee::committee_order(&*committee1),
                &crate::MTCommitteeOrderList { booked_report: vec![0], ..Default::default() }
            );
        }

        // 提交加密信息
        let encrypted_err_info: Vec<u8> =
            hex::decode("01405deeef2a8b0f4a09380d14431dd10fde1ad62b3c27b3fbea4701311d")
                .unwrap()
                .try_into()
                .unwrap();

        assert_ok!(MaintainCommittee::reporter_add_encrypted_error_info(
            RuntimeOrigin::signed(*reporter),
            0,
            *committee1,
            encrypted_err_info.clone()
        ));
        {
            // add_encrypted_err_info:
            // - Writes:
            // CommitteeOps, ReportInfo
            report_info.get_encrypted_info_committee.push(*committee1);
            assert_eq!(MaintainCommittee::report_info(0), Some(report_info));
            committee_ops.encrypted_err_info = Some(encrypted_err_info.clone());
            committee_ops.encrypted_time = 11;
            committee_ops.order_status = crate::MTOrderStatus::Verifying;

            assert_eq!(&MaintainCommittee::committee_ops(&*committee1, 0), &committee_ops);
        }

        // 提交验证Hash
        assert_ok!(MaintainCommittee::committee_submit_verify_hash(
            RuntimeOrigin::signed(*committee1),
            0,
            committee1_hash.clone()
        ));

        // 委员会2预订，提交
        assert_ok!(MaintainCommittee::committee_book_report(RuntimeOrigin::signed(*committee2), 0));
        // 报告人提交加密信息
        assert_ok!(MaintainCommittee::reporter_add_encrypted_error_info(
            RuntimeOrigin::signed(*reporter),
            0,
            *committee2,
            encrypted_err_info.clone()
        ));
        assert_eq!(
            &MaintainCommittee::committee_ops(&*committee2, 0),
            &crate::MTCommitteeOpsDetail {
                booked_time: 11,
                encrypted_err_info: Some(encrypted_err_info.clone()),
                encrypted_time: 11,
                staked_balance: 1000 * ONE_DBC,
                ..Default::default()
            }
        );
        assert_ok!(MaintainCommittee::committee_submit_verify_hash(
            RuntimeOrigin::signed(*committee2),
            0,
            committee2_hash.clone()
        ));

        // 委员会3预订，提交
        assert_ok!(MaintainCommittee::committee_book_report(RuntimeOrigin::signed(*committee3), 0));
        // 报告人提交加密信息
        assert_ok!(MaintainCommittee::reporter_add_encrypted_error_info(
            RuntimeOrigin::signed(*reporter),
            0,
            *committee3,
            encrypted_err_info.clone()
        ));
        assert_ok!(MaintainCommittee::committee_submit_verify_hash(
            RuntimeOrigin::signed(*committee3),
            0,
            committee3_hash.clone()
        ));

        let sorted_committee = vec![*committee2, *committee3, *committee1];
        {
            // submit_confirm_hash:
            // - Writes:
            // CommitteeOrder, CommitteeOps, ReportInfo, LiveReport
            report_info.verifying_committee = None;

            report_info.hashed_committee = sorted_committee.clone();
            report_info.booked_committee = sorted_committee.clone();
            report_info.get_encrypted_info_committee = sorted_committee.clone();
            report_info.hashed_committee = sorted_committee.clone();

            report_info.report_status = crate::ReportStatus::SubmittingRaw;
            assert_eq!(MaintainCommittee::report_info(0), Some(report_info));
            assert_eq!(
                &MaintainCommittee::live_report(),
                &crate::MTLiveReportList { waiting_raw_report: vec![0], ..Default::default() }
            );
            committee_ops.confirm_hash = committee1_hash;
            committee_ops.order_status = crate::MTOrderStatus::WaitingRaw;
            committee_ops.hash_time = 11;
            assert_eq!(&MaintainCommittee::committee_ops(&*committee1, 0), &committee_ops);
            assert_eq!(
                &MaintainCommittee::committee_order(&*committee1),
                &crate::MTCommitteeOrderList { hashed_report: vec![0], ..Default::default() }
            );
        }

        // 3个小时之后才能提交：
        // run_to_block(360 + 13);
        // 立即就可以提交原始值
        {
            report_info.report_status = crate::ReportStatus::SubmittingRaw;
            assert_eq!(MaintainCommittee::report_info(0), Some(report_info));
            assert_eq!(
                &MaintainCommittee::live_report(),
                &crate::MTLiveReportList { waiting_raw_report: vec![0], ..Default::default() }
            );
        }

        // submit_confirm_raw:
        // - Writes:
        // ReportInfo, CommitteeOps
        let extra_err_info = Vec::new();
        assert_ok!(MaintainCommittee::committee_submit_verify_raw(
            RuntimeOrigin::signed(*committee1),
            0,
            machine_id.clone(),
            0,
            reporter_rand_str.clone(),
            committee1_rand_str,
            err_reason.clone(),
            extra_err_info.clone(),
            true
        ));
        // 委员会2,3提交原始值
        assert_ok!(MaintainCommittee::committee_submit_verify_raw(
            RuntimeOrigin::signed(*committee2),
            0,
            machine_id.clone(),
            0,
            reporter_rand_str.clone(),
            committee2_rand_str,
            err_reason.clone(),
            extra_err_info.clone(),
            true
        ));
        assert_ok!(MaintainCommittee::committee_submit_verify_raw(
            RuntimeOrigin::signed(*committee3),
            0,
            machine_id.clone(),
            0,
            reporter_rand_str.clone(),
            committee3_rand_str,
            err_reason.clone(),
            extra_err_info,
            true
        ));

        {
            report_info.confirmed_committee = sorted_committee.clone();
            report_info.support_committee = sorted_committee.clone();
            report_info.machine_id = machine_id.clone();
            report_info.err_info = err_reason;
            assert_eq!(MaintainCommittee::report_info(0), Some(report_info));

            committee_ops.confirm_time = 11;
            committee_ops.confirm_result = true;
            committee_ops.order_status = crate::MTOrderStatus::Finished;

            assert_eq!(&MaintainCommittee::committee_ops(&*committee1, 0), &committee_ops);

            assert_eq!(
                &MaintainCommittee::live_report(),
                &crate::MTLiveReportList { waiting_raw_report: vec![0], ..Default::default() }
            );

            assert!(match report_info.summary() {
                crate::ReportConfirmStatus::Confirmed(..) => true,
                _ => false,
            });
        }

        // assert_eq!(&super::ReportConfirmStatus::Confirmed(_, _, _),
        // MaintainCommittee::summary_report(0));

        run_to_block(360 + 14);

        {
            // summary_fault_case -> summary_waiting_raw -> Confirmed -> mt_machine_offline
            // - Writes:
            // committee_stake; committee_order; LiveReport;
            // report_info.report_status = super::ReportStatus::CommitteeConfirmed;
            assert_eq!(Committee::committee_stake(*committee1).used_stake, 1000 * ONE_DBC);
            assert_eq!(
                MaintainCommittee::committee_order(*committee1),
                crate::MTCommitteeOrderList { finished_report: vec![0], ..Default::default() }
            );
            // assert_eq!(&MachineCommittee::report_info(0), &super::MTReportInfoDetail {
            // ..Default::default() }); assert_eq!(&MaintainCommittee::report_info(0),
            // &report_info);
            assert_eq!(
                &MaintainCommittee::live_report(),
                &crate::MTLiveReportList { finished_report: vec![0], ..Default::default() }
            );

            // mt_machine_offline -> machine_offline
            // - Writes:
            // MachineInfo, LiveMachine, current_era_stash_snap, next_era_stash_snap,
            // current_era_machine_snap, next_era_machine_snap SysInfo, SatshMachine,
            // PosGPUInfo

            assert_eq!(
                &MaintainCommittee::live_report(),
                &crate::MTLiveReportList { finished_report: vec![0], ..Default::default() }
            );
        }

        run_to_block(2880 + 400);

        // 报告人上线机器
        assert_ok!(OnlineProfile::controller_report_online(
            RuntimeOrigin::signed(*controller),
            machine_id.clone()
        ));
    })
}

#[test]
fn report_machine_fault_works_case3() {
    new_test_with_init_params_ext().execute_with(|| {
        let reporter_boxpubkey =
            hex::decode("1e71b5a83ccdeff1592062a1d4da4a272691f08e2024a1ca75a81d534a76210a")
                .unwrap()
                .try_into()
                .unwrap();

        let report_hash: [u8; 16] =
            hex::decode("2611557f5306f050019eeb27648c5494").unwrap().try_into().unwrap();

        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();
        let reporter_rand_str = "abcdef".as_bytes().to_vec();

        let committee1_rand_str = "abc1".as_bytes().to_vec();
        let committee2_rand_str = "abc2".as_bytes().to_vec();
        let committee3_rand_str = "abc3".as_bytes().to_vec();
        let err_reason = "补充信息，可留空".as_bytes().to_vec();
        let committee1_hash: [u8; 16] =
            hex::decode("1c0dad1277a293bdaa4d53bbdc74464d").unwrap().try_into().unwrap();
        let committee2_hash: [u8; 16] =
            hex::decode("ee4102c5ac60e66c7979bd6da07408d1").unwrap().try_into().unwrap();
        let committee3_hash: [u8; 16] =
            hex::decode("d0d71ad755987bb02c2f7fba3e8c46ad").unwrap().try_into().unwrap();

        assert_ok!(MaintainCommittee::report_machine_fault(
            RuntimeOrigin::signed(*reporter),
            crate::MachineFaultType::RentedHardwareMalfunction(report_hash, reporter_boxpubkey),
        ));

        // report_machine hardware fault:
        // - Writes:
        // ReporterStake, ReportInfo, LiveReport, ReporterReport
        let report_status = crate::MTReportInfoDetail {
            reporter: *reporter,
            report_time: 11,
            reporter_stake: 1000 * ONE_DBC, // 15,000,000 / 12,000
            machine_fault_type: crate::MachineFaultType::RentedHardwareMalfunction(
                report_hash,
                reporter_boxpubkey,
            ),
            first_book_time: todo!(),
            machine_id,
            rent_order_id: todo!(),
            err_info: todo!(),
            verifying_committee: todo!(),
            booked_committee: todo!(),
            get_encrypted_info_committee: todo!(),
            hashed_committee: todo!(),
            confirm_start: todo!(),
            confirmed_committee: todo!(),
            support_committee: todo!(),
            against_committee: todo!(),
            report_status: todo!(),
        };
        {
            assert_eq!(MaintainCommittee::report_info(0), Some(report_status));
            assert_eq!(
                &MaintainCommittee::reporter_stake(&*reporter),
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
                &MaintainCommittee::reporter_report(&*reporter),
                &crate::ReporterReportList { processing_report: vec![0], ..Default::default() }
            );
        }

        // 委员会订阅机器故障报告
        assert_ok!(MaintainCommittee::committee_book_report(RuntimeOrigin::signed(*committee1), 0));

        let mut report_info = crate::MTReportInfoDetail {
            first_book_time: 11,
            verifying_committee: Some(committee1.clone()),
            booked_committee: vec![committee1.clone()],
            confirm_start: 11 + 360,
            report_status: crate::ReportStatus::Verifying,
            ..report_status
        };
        let mut committee_ops = crate::MTCommitteeOpsDetail {
            booked_time: 11,
            staked_balance: 1000 * ONE_DBC,
            order_status: crate::MTOrderStatus::WaitingEncrypt,
            ..Default::default()
        };

        {
            // book_fault_order:
            // - Writes:
            // LiveReport, ReportInfo, CommitteeOps, CommitteeOrder
            assert_eq!(
                &MaintainCommittee::live_report(),
                &crate::MTLiveReportList { verifying_report: vec![0], ..Default::default() }
            );
            assert_eq!(MaintainCommittee::report_info(0), Some(report_info));
            assert_eq!(&MaintainCommittee::committee_ops(&*committee1, 0), &committee_ops);
            assert_eq!(
                &MaintainCommittee::committee_order(&*committee1),
                &crate::MTCommitteeOrderList { booked_report: vec![0], ..Default::default() }
            );
        }

        // 提交加密信息
        let encrypted_err_info: Vec<u8> =
            hex::decode("01405deeef2a8b0f4a09380d14431dd10fde1ad62b3c27b3fbea4701311d")
                .unwrap()
                .try_into()
                .unwrap();

        assert_ok!(MaintainCommittee::reporter_add_encrypted_error_info(
            RuntimeOrigin::signed(*reporter),
            0,
            *committee1,
            encrypted_err_info.clone()
        ));
        {
            // add_encrypted_err_info:
            // - Writes:
            // CommitteeOps, ReportInfo
            report_info.get_encrypted_info_committee.push(*committee1);
            assert_eq!(MaintainCommittee::report_info(0), Some(report_info));
            committee_ops.encrypted_err_info = Some(encrypted_err_info.clone());
            committee_ops.encrypted_time = 11;
            committee_ops.order_status = crate::MTOrderStatus::Verifying;

            assert_eq!(&MaintainCommittee::committee_ops(&*committee1, 0), &committee_ops);
        }

        // 提交验证Hash
        assert_ok!(MaintainCommittee::committee_submit_verify_hash(
            RuntimeOrigin::signed(*committee1),
            0,
            committee1_hash.clone()
        ));

        // 委员会2预订，提交
        assert_ok!(MaintainCommittee::committee_book_report(RuntimeOrigin::signed(*committee2), 0));
        // 报告人提交加密信息
        assert_ok!(MaintainCommittee::reporter_add_encrypted_error_info(
            RuntimeOrigin::signed(*reporter),
            0,
            *committee2,
            encrypted_err_info.clone()
        ));
        assert_eq!(
            &MaintainCommittee::committee_ops(&*committee2, 0),
            &crate::MTCommitteeOpsDetail {
                booked_time: 11,
                encrypted_err_info: Some(encrypted_err_info.clone()),
                encrypted_time: 11,
                staked_balance: 1000 * ONE_DBC,
                ..Default::default()
            }
        );
        assert_ok!(MaintainCommittee::committee_submit_verify_hash(
            RuntimeOrigin::signed(*committee2),
            0,
            committee2_hash.clone()
        ));

        // 委员会3预订，提交
        assert_ok!(MaintainCommittee::committee_book_report(RuntimeOrigin::signed(*committee3), 0));
        // 报告人提交加密信息
        assert_ok!(MaintainCommittee::reporter_add_encrypted_error_info(
            RuntimeOrigin::signed(*reporter),
            0,
            *committee3,
            encrypted_err_info.clone()
        ));
        assert_ok!(MaintainCommittee::committee_submit_verify_hash(
            RuntimeOrigin::signed(*committee3),
            0,
            committee3_hash.clone()
        ));

        let sorted_committee = vec![*committee2, *committee3, *committee1];
        {
            // submit_confirm_hash:
            // - Writes:
            // CommitteeOrder, CommitteeOps, ReportInfo, LiveReport
            report_info.verifying_committee = None;

            report_info.hashed_committee = sorted_committee.clone();
            report_info.booked_committee = sorted_committee.clone();
            report_info.get_encrypted_info_committee = sorted_committee.clone();
            report_info.hashed_committee = sorted_committee.clone();

            report_info.report_status = crate::ReportStatus::SubmittingRaw;
            assert_eq!(MaintainCommittee::report_info(0), Some(report_info));
            assert_eq!(
                &MaintainCommittee::live_report(),
                &crate::MTLiveReportList { waiting_raw_report: vec![0], ..Default::default() }
            );
            committee_ops.confirm_hash = committee1_hash;
            committee_ops.order_status = crate::MTOrderStatus::WaitingRaw;
            committee_ops.hash_time = 11;
            assert_eq!(&MaintainCommittee::committee_ops(&*committee1, 0), &committee_ops);
            assert_eq!(
                &MaintainCommittee::committee_order(&*committee1),
                &crate::MTCommitteeOrderList { hashed_report: vec![0], ..Default::default() }
            );
        }

        // 3个小时之后才能提交：
        // run_to_block(360 + 13);
        // 立即就可以提交原始值
        {
            report_info.report_status = crate::ReportStatus::SubmittingRaw;
            assert_eq!(MaintainCommittee::report_info(0), Some(report_info));
            assert_eq!(
                &MaintainCommittee::live_report(),
                &crate::MTLiveReportList { waiting_raw_report: vec![0], ..Default::default() }
            );
        }

        // submit_confirm_raw:
        // - Writes:
        // ReportInfo, CommitteeOps
        let extra_err_info = Vec::new();
        assert_ok!(MaintainCommittee::committee_submit_verify_raw(
            RuntimeOrigin::signed(*committee1),
            0,
            machine_id.clone(),
            0,
            reporter_rand_str.clone(),
            committee1_rand_str,
            err_reason.clone(),
            extra_err_info.clone(),
            false
        ));
        // 委员会2,3提交原始值
        assert_ok!(MaintainCommittee::committee_submit_verify_raw(
            RuntimeOrigin::signed(*committee2),
            0,
            machine_id.clone(),
            0,
            reporter_rand_str.clone(),
            committee2_rand_str,
            err_reason.clone(),
            extra_err_info.clone(),
            false
        ));
        assert_ok!(MaintainCommittee::committee_submit_verify_raw(
            RuntimeOrigin::signed(*committee3),
            0,
            machine_id.clone(),
            0,
            reporter_rand_str.clone(),
            committee3_rand_str,
            err_reason.clone(),
            extra_err_info,
            false
        ));

        {
            report_info.confirmed_committee = sorted_committee.clone();
            report_info.against_committee = sorted_committee.clone();
            report_info.machine_id = machine_id.clone();
            report_info.err_info = err_reason;
            assert_eq!(MaintainCommittee::report_info(0), Some(report_info));

            committee_ops.confirm_time = 11;
            committee_ops.confirm_result = false;
            committee_ops.order_status = crate::MTOrderStatus::Finished;

            assert_eq!(&MaintainCommittee::committee_ops(&*committee1, 0), &committee_ops);

            assert_eq!(
                &MaintainCommittee::live_report(),
                &crate::MTLiveReportList { waiting_raw_report: vec![0], ..Default::default() }
            );

            assert!(match report_info.summary() {
                crate::ReportConfirmStatus::Refuse(..) => true,
                _ => false,
            });
        }

        // assert_eq!(&super::ReportConfirmStatus::Confirmed(_, _, _),
        // MaintainCommittee::summary_report(0));

        // 下一个块即进行summary
        // run_to_block(360 + 14);
        run_to_block(12);

        {
            // summary_fault_case -> summary_waiting_raw -> Confirmed -> mt_machine_offline
            // - Writes:
            // committee_stake; committee_order; LiveReport;
            // report_info.report_status = super::ReportStatus::CommitteeConfirmed;
            assert_eq!(Committee::committee_stake(*committee1).used_stake, 1000 * ONE_DBC);
            assert_eq!(
                MaintainCommittee::committee_order(*committee1),
                crate::MTCommitteeOrderList { finished_report: vec![0], ..Default::default() }
            );
            // assert_eq!(&MachineCommittee::report_info(0), &super::MTReportInfoDetail {
            // ..Default::default() }); assert_eq!(&MaintainCommittee::report_info(0),
            // &report_info);
            assert_eq!(
                &MaintainCommittee::live_report(),
                &crate::MTLiveReportList { finished_report: vec![0], ..Default::default() }
            );

            // mt_machine_offline -> machine_offline
            // - Writes:
            // MachineInfo, LiveMachine, current_era_stash_snap, next_era_stash_snap,
            // current_era_machine_snap, next_era_machine_snap SysInfo, SatshMachine,
            // PosGPUInfo

            assert_eq!(
                &MaintainCommittee::live_report(),
                &crate::MTLiveReportList { finished_report: vec![0], ..Default::default() }
            );

            assert_eq!(MaintainCommittee::unhandled_report_result(11 + 2880 * 2), vec![0]);
        }

        // 将退还质押
        run_to_block(2880 * 2 + 11);
        {
            assert_eq!(
                MaintainCommittee::reporter_stake(&*reporter),
                crate::ReporterStakeInfo { staked_amount: 19000 * ONE_DBC, ..Default::default() }
            );
            assert_eq!(Committee::committee_stake(*committee1).used_stake, 0);
            assert_eq!(Committee::committee_stake(*committee1).staked_amount, 20000 * ONE_DBC);
        }

        // 报告人上线机器
        // 报告被拒绝，机器状态当然是不变
        // assert_ok!(OnlineProfile::controller_report_online(RuntimeOrigin::signed(controller),
        // machine_id.clone()));
    })
}

#[test]
fn report_machine_fault_works_case4() {
    new_test_with_init_params_ext().execute_with(|| {
        let reporter_boxpubkey =
            hex::decode("1e71b5a83ccdeff1592062a1d4da4a272691f08e2024a1ca75a81d534a76210a")
                .unwrap()
                .try_into()
                .unwrap();

        let report_hash: [u8; 16] =
            hex::decode("00e8af0f2ad79a07985e42fa5a045a55").unwrap().try_into().unwrap();

        let committee1_hash: [u8; 16] =
            hex::decode("e8179997b6ba1abe89c7236ebbdf67dd").unwrap().try_into().unwrap();

        assert_ok!(MaintainCommittee::report_machine_fault(
            RuntimeOrigin::signed(*reporter),
            crate::MachineFaultType::RentedHardwareMalfunction(report_hash, reporter_boxpubkey),
        ));

        // report_machine hardware fault:
        // - Writes:
        // ReporterStake, ReportInfo, LiveReport, ReporterReport
        let report_status = crate::MTReportInfoDetail {
            reporter: *reporter,
            report_time: 11,
            reporter_stake: 1000 * ONE_DBC, // 15,000,000 / 12,000
            machine_fault_type: crate::MachineFaultType::RentedHardwareMalfunction(
                report_hash,
                reporter_boxpubkey,
            ),
            first_book_time: todo!(),
            machine_id: todo!(),
            rent_order_id: todo!(),
            err_info: todo!(),
            verifying_committee: todo!(),
            booked_committee: todo!(),
            get_encrypted_info_committee: todo!(),
            hashed_committee: todo!(),
            confirm_start: todo!(),
            confirmed_committee: todo!(),
            support_committee: todo!(),
            against_committee: todo!(),
            report_status: todo!(),
        };
        {
            assert_eq!(MaintainCommittee::report_info(0), Some(report_status));
            assert_eq!(
                &MaintainCommittee::reporter_stake(&*reporter),
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
                &MaintainCommittee::reporter_report(&*reporter),
                &crate::ReporterReportList { processing_report: vec![0], ..Default::default() }
            );
        }

        // 委员会订阅机器故障报告
        assert_ok!(MaintainCommittee::committee_book_report(RuntimeOrigin::signed(*committee1), 0));

        let mut report_info = crate::MTReportInfoDetail {
            first_book_time: 11,
            verifying_committee: Some(committee1.clone()),
            booked_committee: vec![committee1.clone()],
            confirm_start: 11 + 360,
            report_status: crate::ReportStatus::Verifying,
            ..report_status.clone()
        };
        let mut committee_ops = crate::MTCommitteeOpsDetail {
            booked_time: 11,
            staked_balance: 1000 * ONE_DBC,
            order_status: crate::MTOrderStatus::WaitingEncrypt,
            ..Default::default()
        };

        {
            // book_fault_order:
            // - Writes:
            // LiveReport, ReportInfo, CommitteeOps, CommitteeOrder
            assert_eq!(
                &MaintainCommittee::live_report(),
                &crate::MTLiveReportList { verifying_report: vec![0], ..Default::default() }
            );
            assert_eq!(MaintainCommittee::report_info(0), Some(report_info));
            assert_eq!(&MaintainCommittee::committee_ops(&*committee1, 0), &committee_ops);
            assert_eq!(
                &MaintainCommittee::committee_order(&*committee1),
                &crate::MTCommitteeOrderList { booked_report: vec![0], ..Default::default() }
            );
        }

        // 提交加密信息
        let encrypted_err_info: Vec<u8> =
            hex::decode("01405deeef2a8b0f4a09380d14431dd10fde1ad62b3c27b3fbea4701311d")
                .unwrap()
                .try_into()
                .unwrap();

        assert_ok!(MaintainCommittee::reporter_add_encrypted_error_info(
            RuntimeOrigin::signed(*reporter),
            0,
            *committee1,
            encrypted_err_info.clone()
        ));
        {
            // add_encrypted_err_info:
            // - Writes:
            // CommitteeOps, ReportInfo
            report_info.get_encrypted_info_committee.push(*committee1);
            assert_eq!(MaintainCommittee::report_info(0), Some(report_info));
            committee_ops.encrypted_err_info = Some(encrypted_err_info.clone());
            committee_ops.encrypted_time = 11;
            committee_ops.order_status = crate::MTOrderStatus::Verifying;

            assert_eq!(&MaintainCommittee::committee_ops(&*committee1, 0), &committee_ops);
        }

        // 提交验证Hash
        assert_ok!(MaintainCommittee::committee_submit_verify_hash(
            RuntimeOrigin::signed(*committee1),
            0,
            committee1_hash.clone()
        ));

        // 4个小时之后检查状态
        // 将会重新派单，并添加结果，2天后将根据结果进行惩罚！
        run_to_block(480 + 13);
        {
            assert_eq!(
                MaintainCommittee::report_info(0),
                Some(crate::MTReportInfoDetail {
                    first_book_time: 11,
                    verifying_committee: None,
                    booked_committee: vec![*committee1],
                    hashed_committee: vec![*committee1],
                    get_encrypted_info_committee: vec![*committee1],
                    confirm_start: 11 + 360,
                    report_status: crate::ReportStatus::CommitteeConfirmed,
                    ..report_status
                })
            );
            assert_eq!(
                &MaintainCommittee::live_report(),
                &crate::MTLiveReportList {
                    bookable_report: vec![1],
                    // waiting_raw_report: vec![0],
                    ..Default::default()
                }
            );
            assert_eq!(
                MaintainCommittee::report_result(0),
                Some(crate::MTReportResultInfo {
                    report_id: 0,
                    reporter: *reporter,
                    reporter_stake: 0,
                    unruly_committee: vec![*committee1],
                    committee_stake: 1000 * ONE_DBC,
                    slash_time: 11 + 480,
                    slash_exec_time: 11 + 480 + 2880 * 2,
                    report_result: crate::ReportResultType::NoConsensus,
                    slash_result: crate::MCSlashResult::Pending,
                    inconsistent_committee: todo!(),
                    reward_committee: todo!(),
                    machine_stash: todo!(),
                    machine_id: todo!()
                })
            );
        }

        // 不退还报告人第一次质押
        // 惩罚掉委员会的质押
        run_to_block(2880 * 2 + 11 + 4880);
        {
            assert_eq!(
                MaintainCommittee::reporter_stake(&*reporter),
                crate::ReporterStakeInfo {
                    staked_amount: 20000 * ONE_DBC,
                    used_stake: 1000 * ONE_DBC,
                    ..Default::default()
                }
            );
            assert_eq!(Committee::committee_stake(*committee1).used_stake, 0);
            assert_eq!(Committee::committee_stake(*committee1).staked_amount, 19000 * ONE_DBC);
        }
    })
}
