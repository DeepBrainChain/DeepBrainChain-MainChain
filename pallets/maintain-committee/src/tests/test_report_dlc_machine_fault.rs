use super::super::mock::*;
use crate::{Error, MTOrderStatus, ReportStatus, ONE_DLC};
use dbc_support::{
    live_machine::LiveMachine, machine_type::MachineStatus, verify_slash::OPSlashReason, ONE_DAY,
    ONE_MINUTE,
};
use frame_support::{assert_err, assert_ok};
use once_cell::sync::Lazy;
use sp_core::H160;
use std::convert::TryInto;
// 报告机器被租用，但是无法访问
// case1: 只有1委员会预订，同意报告
// case2: 只有1委员会预订，拒绝报告
// case3: 只有1人预订，提交了Hash, 未提交最终结果
// case4: 只有1人预订，未提交Hash, 未提交最终结果

// case5: 有3人预订，都同意报告(最普通的情况)
// case6: 有3人预订，2同意1反对
// case7: 有3人预订，1同意2反对

// case8: 有3人预订，0同意3反对

// case9: 2人预订，都同意
// case10: 2人预订，都反对
// case11: 2人预订，一同意，一反对

const committee: Lazy<sp_core::sr25519::Public> =
    Lazy::new(|| sr25519::Public::from(Sr25519Keyring::One));
const reporter: Lazy<sp_core::sr25519::Public> =
    Lazy::new(|| sr25519::Public::from(Sr25519Keyring::Two));
const machine_stash: Lazy<sp_core::sr25519::Public> =
    Lazy::new(|| sr25519::Public::from(Sr25519Keyring::Ferdie));

const machine_id: Lazy<Vec<u8>> = Lazy::new(|| {
    "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
        .as_bytes()
        .to_vec()
});

#[test]
fn report_dlc_machine_agreed_by_committees_should_works() {
    new_test_with_init_dlc_rent_params_ext().execute_with(|| {
        let asset_id = MaintainCommittee::get_dlc_asset_id();

        let owner = sr25519::Public::from(Sr25519Keyring::Eve);
        let dlc_renter = sr25519::Public::from(Sr25519Keyring::Two);
        let dlc_balance_before_report = Assets::balance(asset_id.into(), dlc_renter);
        let dbc_balance_before_report = Balances::free_balance(dlc_renter);

        let rent_order_infos = RentDlcMachine::machine_rent_order(machine_id.clone());

        let rent_dlc_machine_id = rent_order_infos.rent_order[0];
        assert_eq!(RentDlcMachine::rent_info(rent_dlc_machine_id).is_some(), true);

        let evm_address = H160::default();

        assert_err!(
            MaintainCommittee::report_dlc_machine_fault(
                RuntimeOrigin::signed(owner),
                machine_id.clone(),
                rent_dlc_machine_id,
                evm_address,
            ),
            Error::<TestRuntime>::NotDLCMachineRenter
        );

        // *** pallet_account_id must have some native token to keep alive
        let pallet_account_id = MaintainCommittee::pallet_account_id().unwrap();
        assert_ok!(Balances::transfer(
            RuntimeOrigin::signed(owner),
            pallet_account_id,
            1 * ONE_DBC
        ));

        assert_eq!(Assets::balance(asset_id.into(), dlc_renter), dlc_balance_before_report);

        assert_ok!(MaintainCommittee::report_dlc_machine_fault(
            RuntimeOrigin::signed(dlc_renter.clone()),
            machine_id.clone(),
            rent_dlc_machine_id,
            evm_address,
        ));

        assert_eq!(
            OnlineProfile::offline_machine_to_renters(machine_id.clone()).contains(&dlc_renter),
            true
        );

        assert_eq!(Balances::free_balance(&pallet_account_id), 1 * ONE_DBC);
        assert_eq!(
            Assets::balance(asset_id.into(), dlc_renter),
            dlc_balance_before_report - 10000 * ONE_DLC
        );

        assert_eq!(MaintainCommittee::account_id_2_reserve_dlc(dlc_renter), 10000 * ONE_DLC);

        // 判断调用举报之后的状态
        {
            assert_eq!(
                &MaintainCommittee::live_report(),
                &crate::MTLiveReportList { bookable_report: vec![0], ..Default::default() }
            );
            assert_eq!(
                MaintainCommittee::report_info(0),
                Some(crate::MTReportInfoDetail {
                    reporter: *reporter,
                    report_time: 11,
                    reporter_stake: 0,
                    machine_id: machine_id.clone(),
                    machine_fault_type: crate::MachineFaultType::RentedInaccessible(
                        machine_id.clone(),
                        0
                    ),
                    report_status: crate::ReportStatus::Reported,
                    first_book_time: 0,
                    rent_order_id: 0,
                    err_info: vec![],
                    verifying_committee: None,
                    booked_committee: vec![],
                    get_encrypted_info_committee: vec![],
                    hashed_committee: vec![],
                    confirm_start: BlockNumber::default(),
                    confirmed_committee: vec![],
                    support_committee: vec![],
                    against_committee: vec![]
                })
            );
            assert_eq!(
                &MaintainCommittee::reporter_report(&*reporter),
                &crate::ReporterReportList { processing_report: vec![0], ..Default::default() }
            );

            // TODO: 检查free_balance
            // reporter=committee，因此需要质押40000，减去租用机器的租金
            // assert_eq!(Balances::free_balance(&reporter), INIT_BALANCE - 40000 * ONE_DBC - 10 *
            // ONE_DBC);
        }

        // 委员会订阅机器故障报告
        assert_ok!(MaintainCommittee::committee_book_report(RuntimeOrigin::signed(*committee), 0));

        // 检查订阅之后的状态
        // do_report_machine_fault:
        // - Writes:
        // LiveReport, ReportInfo, CommitteeOps, CommitteeOrder, committee pay txFee
        {
            assert_eq!(
                &MaintainCommittee::live_report(),
                &crate::MTLiveReportList { bookable_report: vec![0], ..Default::default() }
            );
            assert_eq!(
                MaintainCommittee::report_info(0),
                Some(crate::MTReportInfoDetail {
                    reporter: *reporter,
                    report_time: 11,
                    reporter_stake: 0,
                    first_book_time: 11,
                    machine_id: machine_id.clone(),
                    verifying_committee: None,
                    booked_committee: vec![*committee],
                    confirm_start: 11 + 5 * ONE_MINUTE,
                    machine_fault_type: crate::MachineFaultType::RentedInaccessible(
                        machine_id.clone(),
                        0
                    ),
                    report_status: ReportStatus::WaitingBook,
                    rent_order_id: 0,
                    err_info: vec![],
                    get_encrypted_info_committee: vec![],
                    hashed_committee: vec![],
                    confirmed_committee: vec![],
                    support_committee: vec![],
                    against_committee: vec![]
                })
            );
            assert_eq!(
                &MaintainCommittee::committee_ops(&*committee, 0),
                &crate::MTCommitteeOpsDetail {
                    booked_time: 11,
                    order_status: MTOrderStatus::Verifying,
                    ..Default::default()
                }
            );
            assert_eq!(
                &MaintainCommittee::committee_order(&*committee),
                &crate::MTCommitteeOrderList { booked_report: vec![0], ..Default::default() }
            );

            assert_eq!(
                Balances::free_balance(&*committee),
                INIT_BALANCE - 20000 * ONE_DBC - 10 * ONE_DBC
            );
        }

        // 委员会首先提交Hash: 内容为 订单ID + 验证人自己的随机数 + 机器是否有问题
        // hash(0abcd1) => 0x73124a023f585b4018b9ed3593c7470a
        let offline_committee_hash: [u8; 16] =
            hex::decode("73124a023f585b4018b9ed3593c7470a").unwrap().try_into().unwrap();
        // - Writes:
        // LiveReport, CommitteeOps, CommitteeOrder, ReportInfo
        assert_ok!(MaintainCommittee::committee_submit_verify_hash(
            RuntimeOrigin::signed(*committee),
            0,
            offline_committee_hash.clone()
        ));

        // 检查状态
        {
            assert_eq!(
                &MaintainCommittee::live_report(),
                &crate::MTLiveReportList { bookable_report: vec![0], ..Default::default() }
            );
            assert_eq!(
                MaintainCommittee::report_info(0),
                Some(crate::MTReportInfoDetail {
                    reporter: *reporter,
                    report_time: 11,
                    reporter_stake: 0,
                    first_book_time: 11,
                    machine_id: machine_id.clone(),
                    verifying_committee: None,
                    booked_committee: vec![*committee],
                    hashed_committee: vec![*committee],
                    confirm_start: 11 + 5 * ONE_MINUTE,
                    machine_fault_type: crate::MachineFaultType::RentedInaccessible(
                        machine_id.clone(),
                        0
                    ),
                    report_status: ReportStatus::WaitingBook,
                    rent_order_id: 0,
                    err_info: vec![],
                    get_encrypted_info_committee: vec![],
                    confirmed_committee: vec![],
                    support_committee: vec![],
                    against_committee: vec![]
                })
            );
            assert_eq!(
                &MaintainCommittee::committee_ops(&*committee, 0),
                &crate::MTCommitteeOpsDetail {
                    booked_time: 11,
                    confirm_hash: offline_committee_hash,
                    hash_time: 11,
                    order_status: MTOrderStatus::WaitingRaw,
                    ..Default::default()
                }
            );
            assert_eq!(
                &MaintainCommittee::committee_order(&*committee),
                &crate::MTCommitteeOrderList {
                    booked_report: vec![],
                    hashed_report: vec![0],
                    ..Default::default()
                }
            );
        }

        run_to_block(11 + 5 * ONE_MINUTE);
        // - Writes:
        // ReportInfo, committee_ops,
        assert_ok!(MaintainCommittee::committee_submit_inaccessible_raw(
            RuntimeOrigin::signed(*committee),
            0,
            "abcd".as_bytes().to_vec(),
            true
        ));

        // 检查提交了确认信息后的状态
        {
            assert_eq!(
                MaintainCommittee::report_info(0),
                Some(crate::MTReportInfoDetail {
                    reporter: *reporter,
                    report_time: 11,
                    reporter_stake: 0,
                    first_book_time: 11,
                    machine_id: machine_id.clone(),
                    verifying_committee: None,
                    booked_committee: vec![*committee],
                    hashed_committee: vec![*committee],
                    confirmed_committee: vec![*committee],
                    support_committee: vec![*committee],
                    confirm_start: 11 + 5 * ONE_MINUTE,
                    machine_fault_type: crate::MachineFaultType::RentedInaccessible(
                        machine_id.clone(),
                        0
                    ),
                    report_status: ReportStatus::SubmittingRaw,
                    rent_order_id: 0,
                    err_info: vec![],
                    get_encrypted_info_committee: vec![],
                    against_committee: vec![]
                })
            );
            assert_eq!(
                &MaintainCommittee::committee_ops(&*committee, 0),
                &crate::MTCommitteeOpsDetail {
                    booked_time: 11,
                    confirm_hash: offline_committee_hash,
                    hash_time: 11,
                    confirm_time: 12 + 5 * ONE_MINUTE,
                    confirm_result: true,
                    order_status: MTOrderStatus::Finished,
                    ..Default::default()
                }
            );
        }

        run_to_block(12 + 5 * ONE_MINUTE);

        // 检查summary的结果
        // summary_a_inaccessible
        // - Writes:
        // ReportInfo, ReportResult, CommitteeOrder, CommitteeOps
        // LiveReport, UnhandledReportResult, ReporterReport,
        {
            assert_eq!(
                MaintainCommittee::report_info(0),
                Some(crate::MTReportInfoDetail {
                    reporter: *reporter,
                    report_time: 11,
                    reporter_stake: 0,
                    first_book_time: 11,
                    machine_id: machine_id.clone(),
                    verifying_committee: None,
                    booked_committee: vec![*committee],
                    hashed_committee: vec![*committee],
                    confirmed_committee: vec![*committee],
                    support_committee: vec![*committee],
                    confirm_start: 11 + 5 * ONE_MINUTE,
                    machine_fault_type: crate::MachineFaultType::RentedInaccessible(
                        machine_id.clone(),
                        0
                    ),
                    report_status: ReportStatus::CommitteeConfirmed,
                    rent_order_id: 0,
                    err_info: vec![],
                    get_encrypted_info_committee: vec![],
                    against_committee: vec![]
                })
            );
            assert_eq!(
                MaintainCommittee::report_result(0),
                Some(crate::MTReportResultInfo {
                    report_id: 0,
                    reporter: *reporter,
                    reporter_stake: 0,
                    reward_committee: vec![*committee],
                    machine_id: machine_id.clone(),
                    machine_stash: Some(*machine_stash),
                    slash_time: 12 + 5 * ONE_MINUTE,
                    slash_exec_time: 12 + 5 * ONE_MINUTE + ONE_DAY * 2,
                    report_result: crate::ReportResultType::ReportSucceed,
                    slash_result: crate::MCSlashResult::Pending,
                    inconsistent_committee: vec![],
                    unruly_committee: vec![],
                    committee_stake: 0
                })
            );
            assert_eq!(
                &MaintainCommittee::committee_order(&*committee),
                &crate::MTCommitteeOrderList { finished_report: vec![0], ..Default::default() }
            );
            assert_eq!(
                &MaintainCommittee::committee_ops(&*committee, 0),
                &crate::MTCommitteeOpsDetail {
                    booked_time: 11,
                    confirm_hash: offline_committee_hash,
                    hash_time: 11,
                    confirm_time: 12 + 5 * ONE_MINUTE,
                    confirm_result: true,
                    order_status: crate::MTOrderStatus::Finished,

                    ..Default::default()
                }
            );
            assert_eq!(
                &MaintainCommittee::live_report(),
                &crate::MTLiveReportList { finished_report: vec![0], ..Default::default() }
            );
            let unhandled_report_result: Vec<u64> = vec![0];

            assert_eq!(
                &MaintainCommittee::unhandled_report_result(12 + 5 * ONE_MINUTE + ONE_DAY * 2),
                &unhandled_report_result
            );
            assert_eq!(
                &MaintainCommittee::reporter_report(&*reporter),
                &crate::ReporterReportList { succeed_report: vec![0], ..Default::default() }
            );
        }

        {
            assert_eq!(
                OnlineProfile::live_machines(),
                LiveMachine { offline_machine: vec![machine_id.clone()], ..Default::default() }
            );
            let machine_info = OnlineProfile::machines_info(machine_id.clone()).unwrap();
            assert_eq!(
                machine_info.machine_status,
                MachineStatus::ReporterReportOffline(
                    OPSlashReason::RentedInaccessible(11),
                    Box::new(MachineStatus::Rented),
                    *reporter,
                    vec![*committee],
                )
            );
        }

        // run_to_block(1200+(12 + 5 * ONE_MINUTE) + ONE_DAY * 2);
        run_to_block(10 + 12 + 5 * ONE_MINUTE + ONE_DAY * 2);

        let (report_id, evm_address, slash_at) =
            MaintainCommittee::dlc_machine_2_report_info(machine_id.clone()).unwrap();

        assert_eq!(report_id, 0);
        assert_eq!(evm_address, H160::default());
        assert_eq!(slash_at, (12 + 5 * ONE_MINUTE + ONE_DAY * 2) as u64);
        assert_eq!(MaintainCommittee::account_id_2_reserve_dlc(dlc_renter), 0);

        assert_eq!(Assets::balance(asset_id.into(), dlc_renter), dlc_balance_before_report);

        assert_eq!(Balances::free_balance(dlc_renter), dbc_balance_before_report);
        assert_eq!(DlcMachine::dlc_machine_in_staking(machine_id.clone()), false);
    })
}

#[test]
fn report_dlc_machine_refused_by_committees_should_works() {
    new_test_with_init_dlc_rent_params_ext().execute_with(|| {
        let asset_id = MaintainCommittee::get_dlc_asset_id();

        let owner = sr25519::Public::from(Sr25519Keyring::Eve);
        let dlc_renter = sr25519::Public::from(Sr25519Keyring::Two);
        let dlc_balance_before_report = Assets::balance(asset_id.into(), dlc_renter);

        let rent_order_infos = RentDlcMachine::machine_rent_order(machine_id.clone());

        let rent_dlc_machine_id = rent_order_infos.rent_order[0];
        assert_eq!(RentDlcMachine::rent_info(rent_dlc_machine_id).is_some(), true);
        let evm_address = H160::default();

        assert_err!(
            MaintainCommittee::report_dlc_machine_fault(
                RuntimeOrigin::signed(owner),
                machine_id.clone(),
                rent_dlc_machine_id,
                evm_address,
            ),
            Error::<TestRuntime>::NotDLCMachineRenter
        );

        // *** pallet_account_id must have some native token to keep alive
        let pallet_account_id = MaintainCommittee::pallet_account_id().unwrap();
        assert_ok!(Balances::transfer(
            RuntimeOrigin::signed(owner),
            pallet_account_id,
            1 * ONE_DBC
        ));

        assert_eq!(Assets::balance(asset_id.into(), dlc_renter), dlc_balance_before_report);

        assert_ok!(MaintainCommittee::report_dlc_machine_fault(
            RuntimeOrigin::signed(dlc_renter.clone()),
            machine_id.clone(),
            rent_dlc_machine_id,
            evm_address,
        ));

        assert_eq!(
            OnlineProfile::offline_machine_to_renters(machine_id.clone()).contains(&dlc_renter),
            true
        );

        assert_eq!(Balances::free_balance(&pallet_account_id), 1 * ONE_DBC);
        assert_eq!(
            Assets::balance(asset_id.into(), dlc_renter),
            dlc_balance_before_report - 10000 * ONE_DLC
        );

        assert_eq!(MaintainCommittee::account_id_2_reserve_dlc(dlc_renter), 10000 * ONE_DLC);

        // 判断调用举报之后的状态
        {
            assert_eq!(
                &MaintainCommittee::live_report(),
                &crate::MTLiveReportList { bookable_report: vec![0], ..Default::default() }
            );
            assert_eq!(
                MaintainCommittee::report_info(0),
                Some(crate::MTReportInfoDetail {
                    reporter: *reporter,
                    report_time: 11,
                    reporter_stake: 0,
                    machine_id: machine_id.clone(),
                    machine_fault_type: crate::MachineFaultType::RentedInaccessible(
                        machine_id.clone(),
                        0
                    ),
                    report_status: crate::ReportStatus::Reported,
                    first_book_time: 0,
                    rent_order_id: 0,
                    err_info: vec![],
                    verifying_committee: None,
                    booked_committee: vec![],
                    get_encrypted_info_committee: vec![],
                    hashed_committee: vec![],
                    confirm_start: 0,
                    confirmed_committee: vec![],
                    support_committee: vec![],
                    against_committee: vec![]
                })
            );
            assert_eq!(
                &MaintainCommittee::reporter_report(&*reporter),
                &crate::ReporterReportList { processing_report: vec![0], ..Default::default() }
            );
        }

        // 委员会订阅机器故障报告
        assert_ok!(MaintainCommittee::committee_book_report(RuntimeOrigin::signed(*committee), 0));

        // 检查订阅之后的状态
        // do_report_machine_fault:
        // - Writes:
        // LiveReport, ReportInfo, CommitteeOps, CommitteeOrder, committee pay txFee
        {
            assert_eq!(
                &MaintainCommittee::live_report(),
                &crate::MTLiveReportList { bookable_report: vec![0], ..Default::default() }
            );
            assert_eq!(
                MaintainCommittee::report_info(0),
                Some(crate::MTReportInfoDetail {
                    reporter: *reporter,
                    report_time: 11,
                    reporter_stake: 0,
                    first_book_time: 11,
                    machine_id: machine_id.clone(),
                    verifying_committee: None,
                    booked_committee: vec![*committee],
                    confirm_start: 11 + 5 * ONE_MINUTE,
                    machine_fault_type: crate::MachineFaultType::RentedInaccessible(
                        machine_id.clone(),
                        0
                    ),
                    report_status: ReportStatus::WaitingBook,
                    rent_order_id: 0,
                    err_info: vec![],
                    get_encrypted_info_committee: vec![],
                    hashed_committee: vec![],
                    confirmed_committee: vec![],
                    support_committee: vec![],
                    against_committee: vec![]
                })
            );
            assert_eq!(
                &MaintainCommittee::committee_ops(&*committee, 0),
                &crate::MTCommitteeOpsDetail {
                    booked_time: 11,
                    order_status: MTOrderStatus::Verifying,
                    ..Default::default()
                }
            );
            assert_eq!(
                &MaintainCommittee::committee_order(&*committee),
                &crate::MTCommitteeOrderList { booked_report: vec![0], ..Default::default() }
            );

            assert_eq!(
                Balances::free_balance(&*committee),
                INIT_BALANCE - 20000 * ONE_DBC - 10 * ONE_DBC
            );
        }

        // 委员会首先提交Hash: 内容为 订单ID + 验证人自己的随机数 + 机器是否有问题
        // hash(0abcd1) => 0x73124a023f585b4018b9ed3593c7470a
        let offline_committee_hash: [u8; 16] =
            hex::decode("98b18d58d8d3bc2f2037cb8310dd6f0e").unwrap().try_into().unwrap();
        // - Writes:
        // LiveReport, CommitteeOps, CommitteeOrder, ReportInfo
        assert_ok!(MaintainCommittee::committee_submit_verify_hash(
            RuntimeOrigin::signed(*committee),
            0,
            offline_committee_hash.clone()
        ));

        let committee_dlc_balance_before_refuse_report =
            Assets::balance(asset_id.into(), *committee);
        assert_eq!(committee_dlc_balance_before_refuse_report, 0);
        // 检查状态
        {
            assert_eq!(
                &MaintainCommittee::live_report(),
                &crate::MTLiveReportList { bookable_report: vec![0], ..Default::default() }
            );
            assert_eq!(
                MaintainCommittee::report_info(0),
                Some(crate::MTReportInfoDetail {
                    reporter: *reporter,
                    report_time: 11,
                    reporter_stake: 0,
                    first_book_time: 11,
                    machine_id: machine_id.clone(),
                    verifying_committee: None,
                    booked_committee: vec![*committee],
                    hashed_committee: vec![*committee],
                    confirm_start: 11 + 5 * ONE_MINUTE,
                    machine_fault_type: crate::MachineFaultType::RentedInaccessible(
                        machine_id.clone(),
                        0
                    ),
                    report_status: ReportStatus::WaitingBook,
                    rent_order_id: 0,
                    err_info: vec![],
                    get_encrypted_info_committee: vec![],
                    confirmed_committee: vec![],
                    support_committee: vec![],
                    against_committee: vec![]
                })
            );
            assert_eq!(
                &MaintainCommittee::committee_ops(&*committee, 0),
                &crate::MTCommitteeOpsDetail {
                    booked_time: 11,
                    confirm_hash: offline_committee_hash,
                    hash_time: 11,
                    order_status: MTOrderStatus::WaitingRaw,
                    ..Default::default()
                }
            );

            assert_eq!(
                &MaintainCommittee::committee_order(&*committee),
                &crate::MTCommitteeOrderList {
                    booked_report: vec![],
                    hashed_report: vec![0],
                    ..Default::default()
                }
            );
        }

        run_to_block(11 + 5 * ONE_MINUTE);
        // - Writes:
        // ReportInfo, committee_ops,
        assert_ok!(MaintainCommittee::committee_submit_inaccessible_raw(
            RuntimeOrigin::signed(*committee),
            0,
            "fedcba111".as_bytes().to_vec(),
            false
        ));

        // 检查提交了确认信息后的状态
        {
            assert_eq!(
                MaintainCommittee::report_info(0),
                Some(crate::MTReportInfoDetail {
                    reporter: *reporter,
                    report_time: 11,
                    reporter_stake: 0,
                    first_book_time: 11,
                    machine_id: machine_id.clone(),
                    verifying_committee: None,
                    booked_committee: vec![*committee],
                    hashed_committee: vec![*committee],
                    confirmed_committee: vec![*committee],
                    support_committee: vec![],
                    confirm_start: 11 + 5 * ONE_MINUTE,
                    machine_fault_type: crate::MachineFaultType::RentedInaccessible(
                        machine_id.clone(),
                        0
                    ),
                    report_status: ReportStatus::SubmittingRaw,
                    rent_order_id: 0,
                    err_info: vec![],
                    get_encrypted_info_committee: vec![],
                    against_committee: vec![*committee]
                })
            );
            assert_eq!(
                &MaintainCommittee::committee_ops(&*committee, 0),
                &crate::MTCommitteeOpsDetail {
                    booked_time: 11,
                    confirm_hash: offline_committee_hash,
                    hash_time: 11,
                    confirm_time: 12 + 5 * ONE_MINUTE,
                    confirm_result: false,
                    order_status: MTOrderStatus::Finished,
                    ..Default::default()
                }
            );
        }

        run_to_block(12 + 5 * ONE_MINUTE);

        // 检查summary的结果
        // summary_a_inaccessible
        // - Writes:
        // ReportInfo, ReportResult, CommitteeOrder, CommitteeOps
        // LiveReport, UnhandledReportResult, ReporterReport,
        {
            assert_eq!(
                MaintainCommittee::report_info(0),
                Some(crate::MTReportInfoDetail {
                    reporter: *reporter,
                    report_time: 11,
                    reporter_stake: 0,
                    first_book_time: 11,
                    machine_id: machine_id.clone(),
                    verifying_committee: None,
                    booked_committee: vec![*committee],
                    hashed_committee: vec![*committee],
                    confirmed_committee: vec![*committee],
                    support_committee: vec![],
                    confirm_start: 11 + 5 * ONE_MINUTE,
                    machine_fault_type: crate::MachineFaultType::RentedInaccessible(
                        machine_id.clone(),
                        0
                    ),
                    report_status: ReportStatus::CommitteeConfirmed,
                    rent_order_id: 0,
                    err_info: vec![],
                    get_encrypted_info_committee: vec![],
                    against_committee: vec![*committee]
                })
            );
            assert_eq!(
                MaintainCommittee::report_result(0),
                Some(crate::MTReportResultInfo {
                    report_id: 0,
                    reporter: *reporter,
                    reporter_stake: 0,
                    reward_committee: vec![*committee],
                    machine_id: machine_id.clone(),
                    machine_stash: Some(*machine_stash),
                    slash_time: 12 + 5 * ONE_MINUTE,
                    slash_exec_time: 12 + 5 * ONE_MINUTE + ONE_DAY * 2,
                    report_result: crate::ReportResultType::ReportRefused,
                    slash_result: crate::MCSlashResult::Pending,
                    inconsistent_committee: vec![],
                    unruly_committee: vec![],
                    committee_stake: 0
                })
            );
            assert_eq!(
                &MaintainCommittee::committee_order(&*committee),
                &crate::MTCommitteeOrderList { finished_report: vec![0], ..Default::default() }
            );
            assert_eq!(
                &MaintainCommittee::committee_ops(&*committee, 0),
                &crate::MTCommitteeOpsDetail {
                    booked_time: 11,
                    confirm_hash: offline_committee_hash,
                    hash_time: 11,
                    confirm_time: 12 + 5 * ONE_MINUTE,
                    confirm_result: false,
                    order_status: crate::MTOrderStatus::Finished,

                    ..Default::default()
                }
            );
            assert_eq!(
                &MaintainCommittee::live_report(),
                &crate::MTLiveReportList { finished_report: vec![0], ..Default::default() }
            );
            let unhandled_report_result: Vec<u64> = vec![0];
            assert_eq!(
                &MaintainCommittee::unhandled_report_result(12 + 5 * ONE_MINUTE + ONE_DAY * 2),
                &unhandled_report_result
            );
            assert_eq!(
                &MaintainCommittee::reporter_report(&*reporter),
                &crate::ReporterReportList { failed_report: vec![0], ..Default::default() }
            );
        }

        {
            assert_eq!(
                OnlineProfile::live_machines(),
                LiveMachine { rented_machine: vec![machine_id.clone()], ..Default::default() }
            );
            let machine_info = OnlineProfile::machines_info(machine_id.clone()).unwrap();
            assert_eq!(machine_info.machine_status, MachineStatus::Rented);
        }

        run_to_block(10 + 12 + 5 * ONE_MINUTE + ONE_DAY * 2);

        assert_eq!(MaintainCommittee::account_id_2_reserve_dlc(dlc_renter), 0);

        assert_eq!(
            Assets::balance(asset_id.into(), dlc_renter),
            dlc_balance_before_report - 10000 * ONE_DLC
        );
        assert_eq!(
            Assets::balance(asset_id.into(), *committee),
            committee_dlc_balance_before_refuse_report + 10000 * ONE_DLC
        );
        assert_eq!(DlcMachine::dlc_machine_in_staking(machine_id.clone()), true);
    })
}
