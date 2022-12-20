use super::super::{mock::*, *};
use crate::tests::{committee1, committee2, committee4, controller, stash};
use dbc_support::machine_type::{CommitteeUploadInfo, Latitude, Longitude, StakerCustomizeInfo};
use frame_support::assert_ok;
use once_cell::sync::Lazy;
use sp_runtime::Perbill;
use std::convert::TryInto;

const machine_id: Lazy<Vec<u8>> = Lazy::new(|| {
    "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
        .as_bytes()
        .to_vec()
});

// 先正常上线一个机器，再上线第二个机器被拒绝，
// 检查质押的金额（避免质押不足而无法检查出来多惩罚的部分）
#[test]
fn test_machine_online_refused_after_some_online() {
    new_test_with_machine_online().execute_with(|| {
        let machine_id2 = "beacdd9384834e1054fa51d6cb70702685921e91aaf08cbb82ae4c9cb411291a"
            .as_bytes()
            .to_vec();
        let msg = "beacdd9384834e1054fa51d6cb70702685921e91aaf08cbb82ae4c9cb411291a\
                   5CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL";
        let sig = "8a08fb73207c4e8f70257709b2e1cde0ca02e1f73dffa3d0a61044135b7dbb\
                   7aabed4f5b69f9860aae7eee76ecd6551c98f0e4f6d98330d5a39c5ec84c943c8e";

        let server_room = OnlineProfile::stash_server_rooms(&*stash);

        assert_ok!(OnlineProfile::bond_machine(
            Origin::signed(*controller),
            machine_id2.clone(),
            msg.as_bytes().to_vec(),
            hex::decode(sig).unwrap()
        ));

        // 控制账户添加机器信息
        assert_ok!(OnlineProfile::add_machine_info(
            Origin::signed(*controller),
            machine_id2.clone(),
            StakerCustomizeInfo {
                server_room: server_room[0],
                upload_net: 10000,
                download_net: 10000,
                longitude: Longitude::East(1157894),
                latitude: Latitude::North(235678),
                telecom_operators: vec!["China Unicom".into()],
            }
        ));
        // 将会派发机器
        run_to_block(13);

        let machine_info_hash1: [u8; 16] =
            hex::decode("c8ba954825f31240f2985ce458eee1e5").unwrap().try_into().unwrap();
        let machine_info_hash2: [u8; 16] =
            hex::decode("6a98639531dd9197ff5a97b3f2c527aa").unwrap().try_into().unwrap();
        let machine_info_hash3: [u8; 16] =
            hex::decode("5425dd7deff26254321b6682a92254db").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(*committee1),
            machine_id2.clone(),
            machine_info_hash1
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(*committee2),
            machine_id2.clone(),
            machine_info_hash2
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(*committee4),
            machine_id2.clone(),
            machine_info_hash3
        ));

        let mut committee_upload_info = CommitteeUploadInfo {
            machine_id: machine_id2.clone(),
            gpu_type: "GeForceRTX3080".as_bytes().to_vec(),
            gpu_num: 4,
            cuda_core: 8704,
            gpu_mem: 10,
            calc_point: 59890,
            sys_disk: 500,
            data_disk: 3905,
            cpu_type: "Intel(R) Xeon(R) Silver 4214R".as_bytes().to_vec(),
            cpu_core_num: 46,
            cpu_rate: 2400,
            mem_num: 440,

            rand_str: "abcdefg1".as_bytes().to_vec(),
            is_support: false,
        };

        // 委员会提交原始信息
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            Origin::signed(*committee1),
            committee_upload_info.clone()
        ));
        committee_upload_info.rand_str = "abcdefg2".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            Origin::signed(*committee2),
            committee_upload_info.clone()
        ));
        committee_upload_info.rand_str = "abcdefg3".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            Origin::signed(*committee4),
            committee_upload_info
        ));

        run_to_block(15);

        assert_eq!(
            Balances::free_balance(&*stash),
            INIT_BALANCE - 400000 * ONE_DBC - 5000 * ONE_DBC
        );
        assert_eq!(Balances::reserved_balance(&*stash), 400000 * ONE_DBC + 5000 * ONE_DBC);

        // Add two days later stash being slashed:
        assert_eq!(
            OnlineCommittee::pending_slash(0),
            crate::OCPendingSlashInfo {
                machine_id: machine_id2.to_vec(),
                machine_stash: *stash,
                stash_slash_amount: 5000 * ONE_DBC,
                inconsistent_committee: vec![],
                unruly_committee: vec![],
                reward_committee: vec![*committee2, *committee1, *committee4],
                committee_stake: 1000 * ONE_DBC,
                slash_time: 14,
                slash_exec_time: 14 + 2880 * 2,
                book_result: OCBookResultType::OnlineRefused,
                slash_result: OCSlashResult::Pending,
            }
        );

        // on_initialize will do slash
        run_to_block(15 + 2880 * 2);

        assert_eq!(
            Balances::free_balance(&*stash),
            INIT_BALANCE - 400000 * ONE_DBC - 5000 * ONE_DBC
        );
        assert_eq!(Balances::reserved_balance(&*stash), 400000 * ONE_DBC);
        assert!(<PendingSlash<TestRuntime>>::contains_key(0));

        assert_eq!(
            Balances::free_balance(*committee1),
            INIT_BALANCE - 20000 * ONE_DBC +
                Perbill::from_rational_approximation(1u32, 3u32) * (5000 * ONE_DBC)
        );
    })
}

// 3票拒绝,机器上线失败，质押将会被扣除5%, 剩余会被退还
#[test]
fn test_machine_online_refused_claim_reserved() {
    new_test_with_online_machine_distribution().execute_with(|| {
        let committee1_box_pubkey =
            hex::decode("ff3033c763f71bc51f372c1dc5095accc26880e138df84cac13c46bfd7dbd74f")
                .unwrap()
                .try_into()
                .unwrap();

        let machine_info_hash1: [u8; 16] =
            hex::decode("cee14a520ba6a988c306aab9dc3794b1").unwrap().try_into().unwrap();
        let machine_info_hash2: [u8; 16] =
            hex::decode("8c7e7ca563169689f1c789f8d4f510f8").unwrap().try_into().unwrap();
        let machine_info_hash3: [u8; 16] =
            hex::decode("73af18cb31a2ebbea4eab9e9e519539e").unwrap().try_into().unwrap();

        assert_eq!(
            OnlineCommittee::machine_committee(&*machine_id),
            OCMachineCommitteeList {
                book_time: 6,
                booked_committee: vec![*committee2, *committee1, *committee4],
                confirm_start_time: 6 + 4320,
                status: OCVerifyStatus::SubmittingHash,
                ..Default::default()
            }
        );

        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(*committee1),
            machine_id.clone(),
            machine_info_hash1
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(*committee2),
            machine_id.clone(),
            machine_info_hash2
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(*committee4),
            machine_id.clone(),
            machine_info_hash3
        ));

        let mut committee_upload_info = CommitteeUploadInfo {
            machine_id: machine_id.clone(),
            gpu_type: "GeForceRTX3080".as_bytes().to_vec(),
            gpu_num: 4,
            cuda_core: 8704,
            gpu_mem: 10,
            calc_point: 59890,
            sys_disk: 500,
            data_disk: 3905,
            cpu_type: "Intel(R) Xeon(R) Silver 4214R".as_bytes().to_vec(),
            cpu_core_num: 46,
            cpu_rate: 2400,
            mem_num: 440,

            rand_str: "abcdefg1".as_bytes().to_vec(),
            is_support: false,
        };

        // 委员会提交原始信息
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            Origin::signed(*committee1),
            committee_upload_info.clone()
        ));
        committee_upload_info.rand_str = "abcdefg2".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            Origin::signed(*committee2),
            committee_upload_info.clone()
        ));
        committee_upload_info.rand_str = "abcdefg3".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            Origin::signed(*committee4),
            committee_upload_info
        ));

        run_to_block(11);

        assert_eq!(Balances::free_balance(&*stash), INIT_BALANCE - 5000 * ONE_DBC);
        assert_eq!(Balances::reserved_balance(&*stash), 5000 * ONE_DBC);

        // Add two days later stash being slashed:
        assert_eq!(
            OnlineCommittee::pending_slash(0),
            crate::OCPendingSlashInfo {
                machine_id: machine_id.to_vec(),
                machine_stash: *stash,
                stash_slash_amount: 5000 * ONE_DBC,
                inconsistent_committee: vec![],
                unruly_committee: vec![],
                reward_committee: vec![*committee2, *committee1, *committee4],
                committee_stake: 1000 * ONE_DBC,
                slash_time: 11,
                slash_exec_time: 11 + 2880 * 2,
                book_result: OCBookResultType::OnlineRefused,
                slash_result: OCSlashResult::Pending,
            }
        );

        // 5771 on_initialize will do slash
        run_to_block(11 + 2880 * 2);

        assert_eq!(Balances::free_balance(&*stash), INIT_BALANCE - 5000 * ONE_DBC);
        assert_eq!(Balances::reserved_balance(&*stash), 0);
        assert!(<PendingSlash<TestRuntime>>::contains_key(0));

        assert_eq!(
            Balances::free_balance(*committee1),
            INIT_BALANCE - 20000 * ONE_DBC +
                Perbill::from_rational_approximation(1u32, 3u32) * (5000 * ONE_DBC)
        );

        assert_eq!(Balances::reserved_balance(*committee1), 20000 * ONE_DBC);
        assert_eq!(
            Committee::committee_stake(*committee1),
            committee::CommitteeStakeInfo {
                box_pubkey: committee1_box_pubkey,
                staked_amount: 20000 * ONE_DBC,
                used_stake: 0,
                ..Default::default()
            }
        );
    })
}

// 3票拒绝，机器上线失败，stash申述，技术委员会未处理
#[test]
fn test_online_refused_apply_review_ignored_works() {
    new_test_with_online_machine_distribution().execute_with(|| {
        let committee1_box_pubkey =
            hex::decode("ff3033c763f71bc51f372c1dc5095accc26880e138df84cac13c46bfd7dbd74f")
                .unwrap()
                .try_into()
                .unwrap();

        let machine_info_hash1: [u8; 16] =
            hex::decode("cee14a520ba6a988c306aab9dc3794b1").unwrap().try_into().unwrap();
        let machine_info_hash2: [u8; 16] =
            hex::decode("8c7e7ca563169689f1c789f8d4f510f8").unwrap().try_into().unwrap();
        let machine_info_hash3: [u8; 16] =
            hex::decode("73af18cb31a2ebbea4eab9e9e519539e").unwrap().try_into().unwrap();

        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(*committee1),
            machine_id.clone(),
            machine_info_hash1
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(*committee2),
            machine_id.clone(),
            machine_info_hash2
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(*committee4),
            machine_id.clone(),
            machine_info_hash3
        ));

        let mut committee_upload_info = CommitteeUploadInfo {
            machine_id: machine_id.to_vec(),
            gpu_type: "GeForceRTX3080".as_bytes().to_vec(),
            gpu_num: 4,
            cuda_core: 8704,
            gpu_mem: 10,
            calc_point: 59890,
            sys_disk: 500,
            data_disk: 3905,
            cpu_type: "Intel(R) Xeon(R) Silver 4214R".as_bytes().to_vec(),
            cpu_core_num: 46,
            cpu_rate: 2400,
            mem_num: 440,

            rand_str: "abcdefg1".as_bytes().to_vec(),
            is_support: false,
        };

        // 委员会提交原始信息
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            Origin::signed(*committee1),
            committee_upload_info.clone()
        ));
        committee_upload_info.rand_str = "abcdefg2".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            Origin::signed(*committee2),
            committee_upload_info.clone()
        ));
        committee_upload_info.rand_str = "abcdefg3".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            Origin::signed(*committee4),
            committee_upload_info
        ));

        run_to_block(11);

        // FIXME: 这里改为stash
        assert_ok!(OnlineCommittee::apply_slash_review(Origin::signed(*controller), 0, vec![]));
        assert_eq!(Balances::reserved_balance(*stash), 5000 * ONE_DBC + 1000 * ONE_DBC);
        assert_eq!(OnlineProfile::stash_stake(*stash), 5000 * ONE_DBC + 1000 * ONE_DBC);
        assert_eq!(
            OnlineCommittee::pending_slash_review(0),
            OCPendingSlashReviewInfo {
                applicant: *stash,
                staked_amount: 1000 * ONE_DBC,
                apply_time: 12,
                expire_time: 11 + 2880 * 2,
                reason: vec![],
            }
        );

        run_to_block(11 + 2880 * 2);

        assert_eq!(Balances::reserved_balance(*stash), 0);
        assert_eq!(Balances::free_balance(*stash), INIT_BALANCE - 6000 * ONE_DBC);

        assert_eq!(
            Balances::free_balance(*committee1),
            INIT_BALANCE - 20000 * ONE_DBC +
                Perbill::from_rational_approximation(1u32, 3u32) * (5000 * ONE_DBC)
        );
        assert_eq!(
            Committee::committee_stake(*committee1),
            committee::CommitteeStakeInfo {
                box_pubkey: committee1_box_pubkey,
                staked_amount: 20000 * ONE_DBC,
                used_stake: 0,
                ..Default::default()
            }
        );

        assert!(!<PendingSlashReview<TestRuntime>>::contains_key(0));
    })
}

// 3票拒绝，机器上线失败，stash申述，技术委员会同意
#[test]
fn test_online_refused_apply_review_succeed_works() {
    new_test_with_online_machine_distribution().execute_with(|| {
        let committee1_box_pubkey =
            hex::decode("ff3033c763f71bc51f372c1dc5095accc26880e138df84cac13c46bfd7dbd74f")
                .unwrap()
                .try_into()
                .unwrap();

        let machine_info_hash1: [u8; 16] =
            hex::decode("cee14a520ba6a988c306aab9dc3794b1").unwrap().try_into().unwrap();
        let machine_info_hash2: [u8; 16] =
            hex::decode("8c7e7ca563169689f1c789f8d4f510f8").unwrap().try_into().unwrap();
        let machine_info_hash3: [u8; 16] =
            hex::decode("73af18cb31a2ebbea4eab9e9e519539e").unwrap().try_into().unwrap();

        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(*committee1),
            machine_id.clone(),
            machine_info_hash1
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(*committee2),
            machine_id.clone(),
            machine_info_hash2
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(*committee4),
            machine_id.clone(),
            machine_info_hash3
        ));

        let mut committee_upload_info = CommitteeUploadInfo {
            machine_id: machine_id.to_vec(),
            gpu_type: "GeForceRTX3080".as_bytes().to_vec(),
            gpu_num: 4,
            cuda_core: 8704,
            gpu_mem: 10,
            calc_point: 59890,
            sys_disk: 500,
            data_disk: 3905,
            cpu_type: "Intel(R) Xeon(R) Silver 4214R".as_bytes().to_vec(),
            cpu_core_num: 46,
            cpu_rate: 2400,
            mem_num: 440,

            rand_str: "abcdefg1".as_bytes().to_vec(),
            is_support: false,
        };

        // 委员会提交原始信息
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            Origin::signed(*committee1),
            committee_upload_info.clone()
        ));
        committee_upload_info.rand_str = "abcdefg2".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            Origin::signed(*committee2),
            committee_upload_info.clone()
        ));
        committee_upload_info.rand_str = "abcdefg3".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            Origin::signed(*committee4),
            committee_upload_info.clone()
        ));

        run_to_block(11);

        // stash申诉
        assert_ok!(OnlineCommittee::apply_slash_review(Origin::signed(*controller), 0, vec![]));

        // 申诉时的状态
        {
            assert_eq!(Balances::reserved_balance(*stash), 5000 * ONE_DBC + 1000 * ONE_DBC);
            assert_eq!(OnlineProfile::stash_stake(*stash), 5000 * ONE_DBC + 1000 * ONE_DBC);
            assert_eq!(
                OnlineCommittee::pending_slash_review(0),
                OCPendingSlashReviewInfo {
                    applicant: *stash,
                    staked_amount: 1000 * ONE_DBC,
                    apply_time: 12,
                    expire_time: 11 + 2880 * 2,
                    reason: vec![],
                }
            );
        }

        assert_ok!(OnlineCommittee::do_cancel_slash(0));

        // 申诉被取消后的状态
        assert_eq!(Balances::reserved_balance(*stash), 0);
        assert_eq!(Balances::free_balance(*stash), INIT_BALANCE + 3000 * ONE_DBC);
        assert_eq!(Balances::free_balance(*committee1), INIT_BALANCE - 20000 * ONE_DBC);
        assert_eq!(
            Committee::committee_stake(*committee1),
            committee::CommitteeStakeInfo {
                box_pubkey: committee1_box_pubkey,
                staked_amount: 19000 * ONE_DBC,
                used_stake: 0,
                ..Default::default()
            }
        );

        assert!(!<PendingSlashReview<TestRuntime>>::contains_key(0));
    })
}

// 2票拒绝，1票支持，机器上线失败，stash申诉，技术委员会没通过
#[test]
fn test_online_refused_1_2_apply_review_failed_works() {
    new_test_with_online_machine_distribution().execute_with(|| {
        // let committee1_box_pubkey =
        // hex::decode("ff3033c763f71bc51f372c1dc5095accc26880e138df84cac13c46bfd7dbd74f")
        //     .unwrap()
        //     .try_into()
        //     .unwrap();

        let machine_info_hash1: [u8; 16] =
            hex::decode("cee14a520ba6a988c306aab9dc3794b1").unwrap().try_into().unwrap();
        let machine_info_hash2: [u8; 16] =
            hex::decode("8c7e7ca563169689f1c789f8d4f510f8").unwrap().try_into().unwrap();
        let machine_info_hash3: [u8; 16] =
            hex::decode("4a6b2df1e1a77b9bcdab5e31dc7950d2").unwrap().try_into().unwrap();

        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(*committee1),
            machine_id.clone(),
            machine_info_hash1
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(*committee2),
            machine_id.clone(),
            machine_info_hash2
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(*committee4),
            machine_id.clone(),
            machine_info_hash3
        ));

        let mut committee_upload_info = CommitteeUploadInfo {
            machine_id: machine_id.to_vec(),
            gpu_type: "GeForceRTX3080".as_bytes().to_vec(),
            gpu_num: 4,
            cuda_core: 8704,
            gpu_mem: 10,
            calc_point: 59890,
            sys_disk: 500,
            data_disk: 3905,
            cpu_type: "Intel(R) Xeon(R) Silver 4214R".as_bytes().to_vec(),
            cpu_core_num: 46,
            cpu_rate: 2400,
            mem_num: 440,

            rand_str: "abcdefg1".as_bytes().to_vec(),
            is_support: false,
        };

        // 委员会提交原始信息
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            Origin::signed(*committee1),
            committee_upload_info.clone()
        ));
        committee_upload_info.rand_str = "abcdefg2".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            Origin::signed(*committee2),
            committee_upload_info.clone()
        ));
        committee_upload_info.rand_str = "abcdefg3".as_bytes().to_vec();
        committee_upload_info.is_support = true;
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            Origin::signed(*committee4),
            committee_upload_info
        ));

        run_to_block(11);

        // stash申诉
        assert_ok!(OnlineCommittee::apply_slash_review(Origin::signed(*controller), 0, vec![]));

        // TODO: add this
    })
}

// 2票拒绝，1票支持，机器上线失败，committee申诉，技术委员会通过
#[test]
fn test_online_refused_1_2_apply_review_succeed_works() {
    new_test_with_online_machine_distribution().execute_with(|| {
        let machine_info_hash1: [u8; 16] =
            hex::decode("cee14a520ba6a988c306aab9dc3794b1").unwrap().try_into().unwrap();
        let machine_info_hash2: [u8; 16] =
            hex::decode("8c7e7ca563169689f1c789f8d4f510f8").unwrap().try_into().unwrap();
        let machine_info_hash3: [u8; 16] =
            hex::decode("73af18cb31a2ebbea4eab9e9e519539e").unwrap().try_into().unwrap();

        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(*committee1),
            machine_id.clone(),
            machine_info_hash1
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(*committee2),
            machine_id.clone(),
            machine_info_hash2
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(*committee4),
            machine_id.clone(),
            machine_info_hash3
        ));

        let mut committee_upload_info = CommitteeUploadInfo {
            machine_id: machine_id.to_vec(),
            gpu_type: "GeForceRTX3080".as_bytes().to_vec(),
            gpu_num: 4,
            cuda_core: 8704,
            gpu_mem: 10,
            calc_point: 59890,
            sys_disk: 500,
            data_disk: 3905,
            cpu_type: "Intel(R) Xeon(R) Silver 4214R".as_bytes().to_vec(),
            cpu_core_num: 46,
            cpu_rate: 2400,
            mem_num: 440,

            rand_str: "abcdefg1".as_bytes().to_vec(),
            is_support: false,
        };

        // 委员会提交原始信息
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            Origin::signed(*committee1),
            committee_upload_info.clone()
        ));
        committee_upload_info.rand_str = "abcdefg2".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            Origin::signed(*committee2),
            committee_upload_info.clone()
        ));
        committee_upload_info.rand_str = "abcdefg3".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            Origin::signed(*committee4),
            committee_upload_info
        ));

        run_to_block(11);

        // stash申诉
        assert_ok!(OnlineCommittee::apply_slash_review(Origin::signed(*controller), 0, vec![]));

        // TODO: add this
    })
}
