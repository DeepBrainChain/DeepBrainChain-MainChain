use super::super::{mock::*, *};
use frame_support::assert_ok;
use online_profile::CommitteeUploadInfo;
use sp_runtime::Perbill;
use std::convert::TryInto;

// 3票拒绝,机器上线失败，质押将会被扣除5%, 剩余会被退还
#[test]
fn test_machine_online_refused_claim_reserved() {
    new_test_with_online_machine_distribution().execute_with(|| {
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();
        let _controller: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();
        let stash: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();

        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Alice).into();
        let committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Charlie).into();
        // let committee3: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Dave).into();
        let committee4: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();
        let committee1_box_pubkey = hex::decode("ff3033c763f71bc51f372c1dc5095accc26880e138df84cac13c46bfd7dbd74f")
            .unwrap()
            .try_into()
            .unwrap();

        let machine_info_hash1: [u8; 16] = hex::decode("cee14a520ba6a988c306aab9dc3794b1").unwrap().try_into().unwrap();
        let machine_info_hash2: [u8; 16] = hex::decode("8c7e7ca563169689f1c789f8d4f510f8").unwrap().try_into().unwrap();
        let machine_info_hash3: [u8; 16] = hex::decode("73af18cb31a2ebbea4eab9e9e519539e").unwrap().try_into().unwrap();

        assert_eq!(
            OnlineCommittee::machine_committee(&machine_id),
            OCMachineCommitteeList {
                book_time: 6,
                booked_committee: vec![committee2, committee1, committee4],
                confirm_start_time: 6 + 4320,
                status: OCVerifyStatus::SubmittingHash,
                ..Default::default()
            }
        );

        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(committee1),
            machine_id.clone(),
            machine_info_hash1
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(committee2),
            machine_id.clone(),
            machine_info_hash2
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(committee4),
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
        assert_ok!(OnlineCommittee::submit_confirm_raw(Origin::signed(committee1), committee_upload_info.clone()));
        committee_upload_info.rand_str = "abcdefg2".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(Origin::signed(committee2), committee_upload_info.clone()));
        committee_upload_info.rand_str = "abcdefg3".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(Origin::signed(committee4), committee_upload_info.clone()));

        run_to_block(11);

        assert_eq!(Balances::free_balance(&stash), INIT_BALANCE - 5000 * ONE_DBC);
        assert_eq!(Balances::reserved_balance(&stash), 5000 * ONE_DBC);

        // Add two days later stash being slashed:
        assert_eq!(
            OnlineCommittee::pending_slash(0),
            crate::OCPendingSlashInfo {
                machine_id,
                machine_stash: stash,
                stash_slash_amount: 5000 * ONE_DBC,
                inconsistent_committee: vec![],
                unruly_committee: vec![],
                reward_committee: vec![committee2, committee1, committee4],
                committee_stake: 1000 * ONE_DBC,
                slash_time: 11,
                slash_exec_time: 11 + 2880 * 2,
                book_result: OCBookResultType::OnlineRefused,
                slash_result: OCSlashResult::Pending,
            }
        );

        // 5771 on_initialize will do slash
        run_to_block(11 + 2880 * 2);

        assert_eq!(Balances::free_balance(&stash), INIT_BALANCE - 5000 * ONE_DBC);
        assert_eq!(Balances::reserved_balance(&stash), 0);
        assert_eq!(<PendingSlash<TestRuntime>>::contains_key(0), true);

        assert_eq!(
            Balances::free_balance(committee1),
            INIT_BALANCE - 20000 * ONE_DBC + Perbill::from_rational_approximation(1u32, 3u32) * (5000 * ONE_DBC)
        );

        assert_eq!(Balances::reserved_balance(committee1), 20000 * ONE_DBC);
        assert_eq!(
            Committee::committee_stake(committee1),
            committee::CommitteeStakeInfo {
                box_pubkey: committee1_box_pubkey,
                staked_amount: 20000 * ONE_DBC,
                used_stake: 0 * ONE_DBC,
                ..Default::default()
            }
        );
    })
}

// 3票拒绝，机器上线失败，stash申述，技术委员会未处理
#[test]
fn test_online_refused_apply_review_ignored_works() {
    new_test_with_online_machine_distribution().execute_with(|| {
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();
        let controller: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();
        let stash: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();

        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Alice).into();
        let committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Charlie).into();
        // let committee3: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Dave).into();
        let committee4: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();
        let committee1_box_pubkey = hex::decode("ff3033c763f71bc51f372c1dc5095accc26880e138df84cac13c46bfd7dbd74f")
            .unwrap()
            .try_into()
            .unwrap();

        let machine_info_hash1: [u8; 16] = hex::decode("cee14a520ba6a988c306aab9dc3794b1").unwrap().try_into().unwrap();
        let machine_info_hash2: [u8; 16] = hex::decode("8c7e7ca563169689f1c789f8d4f510f8").unwrap().try_into().unwrap();
        let machine_info_hash3: [u8; 16] = hex::decode("73af18cb31a2ebbea4eab9e9e519539e").unwrap().try_into().unwrap();

        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(committee1),
            machine_id.clone(),
            machine_info_hash1
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(committee2),
            machine_id.clone(),
            machine_info_hash2
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(committee4),
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
        assert_ok!(OnlineCommittee::submit_confirm_raw(Origin::signed(committee1), committee_upload_info.clone()));
        committee_upload_info.rand_str = "abcdefg2".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(Origin::signed(committee2), committee_upload_info.clone()));
        committee_upload_info.rand_str = "abcdefg3".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(Origin::signed(committee4), committee_upload_info.clone()));

        run_to_block(11);

        // FIXME: 这里改为stash
        assert_ok!(OnlineCommittee::apply_slash_review(Origin::signed(controller), 0, vec![]));
        assert_eq!(Balances::reserved_balance(stash), 5000 * ONE_DBC + 1000 * ONE_DBC);
        assert_eq!(OnlineProfile::stash_stake(stash), 5000 * ONE_DBC + 1000 * ONE_DBC);
        assert_eq!(
            OnlineCommittee::pending_slash_review(0),
            OCPendingSlashReviewInfo {
                applicant: stash,
                staked_amount: 1000 * ONE_DBC,
                apply_time: 12,
                expire_time: 11 + 2880 * 2,
                reason: vec![],
            }
        );

        run_to_block(11 + 2880 * 2);

        assert_eq!(Balances::reserved_balance(stash), 0);
        assert_eq!(Balances::free_balance(stash), INIT_BALANCE - 6000 * ONE_DBC);

        assert_eq!(
            Balances::free_balance(committee1),
            INIT_BALANCE - 20000 * ONE_DBC + Perbill::from_rational_approximation(1u32, 3u32) * (5000 * ONE_DBC)
        );
        assert_eq!(
            Committee::committee_stake(committee1),
            committee::CommitteeStakeInfo {
                box_pubkey: committee1_box_pubkey,
                staked_amount: 20000 * ONE_DBC,
                used_stake: 0 * ONE_DBC,
                ..Default::default()
            }
        );

        assert_eq!(<PendingSlashReview<TestRuntime>>::contains_key(0), false);
    })
}

// 3票拒绝，机器上线失败，stash申述，技术委员会同意
#[test]
fn test_online_refused_apply_review_succeed_works() {
    new_test_with_online_machine_distribution().execute_with(|| {
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();
        let controller: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();
        let stash: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();

        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Alice).into();
        let committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Charlie).into();
        // let committee3: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Dave).into();
        let committee4: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();
        let committee1_box_pubkey = hex::decode("ff3033c763f71bc51f372c1dc5095accc26880e138df84cac13c46bfd7dbd74f")
            .unwrap()
            .try_into()
            .unwrap();

        let machine_info_hash1: [u8; 16] = hex::decode("cee14a520ba6a988c306aab9dc3794b1").unwrap().try_into().unwrap();
        let machine_info_hash2: [u8; 16] = hex::decode("8c7e7ca563169689f1c789f8d4f510f8").unwrap().try_into().unwrap();
        let machine_info_hash3: [u8; 16] = hex::decode("73af18cb31a2ebbea4eab9e9e519539e").unwrap().try_into().unwrap();

        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(committee1),
            machine_id.clone(),
            machine_info_hash1
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(committee2),
            machine_id.clone(),
            machine_info_hash2
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(committee4),
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
        assert_ok!(OnlineCommittee::submit_confirm_raw(Origin::signed(committee1), committee_upload_info.clone()));
        committee_upload_info.rand_str = "abcdefg2".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(Origin::signed(committee2), committee_upload_info.clone()));
        committee_upload_info.rand_str = "abcdefg3".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(Origin::signed(committee4), committee_upload_info.clone()));

        run_to_block(11);

        // stash申诉
        assert_ok!(OnlineCommittee::apply_slash_review(Origin::signed(controller), 0, vec![]));

        // 申诉时的状态
        assert_eq!(Balances::reserved_balance(stash), 5000 * ONE_DBC + 1000 * ONE_DBC);
        assert_eq!(OnlineProfile::stash_stake(stash), 5000 * ONE_DBC + 1000 * ONE_DBC);
        assert_eq!(
            OnlineCommittee::pending_slash_review(0),
            OCPendingSlashReviewInfo {
                applicant: stash,
                staked_amount: 1000 * ONE_DBC,
                apply_time: 12,
                expire_time: 11 + 2880 * 2,
                reason: vec![],
            }
        );

        assert_ok!(OnlineCommittee::do_cancel_slash(0));

        // 申诉被取消后的状态
        assert_eq!(Balances::reserved_balance(stash), 0);
        assert_eq!(Balances::free_balance(stash), INIT_BALANCE + 3000 * ONE_DBC);
        assert_eq!(Balances::free_balance(committee1), INIT_BALANCE - 20000 * ONE_DBC);
        assert_eq!(
            Committee::committee_stake(committee1),
            committee::CommitteeStakeInfo {
                box_pubkey: committee1_box_pubkey,
                staked_amount: 19000 * ONE_DBC,
                used_stake: 0 * ONE_DBC,
                ..Default::default()
            }
        );

        assert_eq!(<PendingSlashReview<TestRuntime>>::contains_key(0), false);
    })
}

// 2票拒绝，1票支持，机器上线失败，stash申诉，技术委员会没通过
#[test]
fn test_online_refused_1_2_apply_review_failed_works() {
    new_test_with_online_machine_distribution().execute_with(|| {
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();
        let controller: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();
        let _stash: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();

        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Alice).into();
        let committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Charlie).into();
        // let committee3: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Dave).into();
        let committee4: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();
        // let committee1_box_pubkey = hex::decode("ff3033c763f71bc51f372c1dc5095accc26880e138df84cac13c46bfd7dbd74f")
        //     .unwrap()
        //     .try_into()
        //     .unwrap();

        let machine_info_hash1: [u8; 16] = hex::decode("cee14a520ba6a988c306aab9dc3794b1").unwrap().try_into().unwrap();
        let machine_info_hash2: [u8; 16] = hex::decode("8c7e7ca563169689f1c789f8d4f510f8").unwrap().try_into().unwrap();
        let machine_info_hash3: [u8; 16] = hex::decode("4a6b2df1e1a77b9bcdab5e31dc7950d2").unwrap().try_into().unwrap();

        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(committee1),
            machine_id.clone(),
            machine_info_hash1
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(committee2),
            machine_id.clone(),
            machine_info_hash2
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(committee4),
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
        assert_ok!(OnlineCommittee::submit_confirm_raw(Origin::signed(committee1), committee_upload_info.clone()));
        committee_upload_info.rand_str = "abcdefg2".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(Origin::signed(committee2), committee_upload_info.clone()));
        committee_upload_info.rand_str = "abcdefg3".as_bytes().to_vec();
        committee_upload_info.is_support = true;
        assert_ok!(OnlineCommittee::submit_confirm_raw(Origin::signed(committee4), committee_upload_info.clone()));

        run_to_block(11);

        // stash申诉
        assert_ok!(OnlineCommittee::apply_slash_review(Origin::signed(controller), 0, vec![]));

        // TODO: add this
    })
}

// 2票拒绝，1票支持，机器上线失败，committee申诉，技术委员会通过
#[test]
fn test_online_refused_1_2_apply_review_succeed_works() {
    new_test_with_online_machine_distribution().execute_with(|| {
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();
        let controller: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();
        let _stash: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();

        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Alice).into();
        let committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Charlie).into();
        // let committee3: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Dave).into();
        let committee4: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();
        // let committee1_box_pubkey = hex::decode("ff3033c763f71bc51f372c1dc5095accc26880e138df84cac13c46bfd7dbd74f")
        //     .unwrap()
        //     .try_into()
        //     .unwrap();

        let machine_info_hash1: [u8; 16] = hex::decode("cee14a520ba6a988c306aab9dc3794b1").unwrap().try_into().unwrap();
        let machine_info_hash2: [u8; 16] = hex::decode("8c7e7ca563169689f1c789f8d4f510f8").unwrap().try_into().unwrap();
        let machine_info_hash3: [u8; 16] = hex::decode("73af18cb31a2ebbea4eab9e9e519539e").unwrap().try_into().unwrap();

        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(committee1),
            machine_id.clone(),
            machine_info_hash1
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(committee2),
            machine_id.clone(),
            machine_info_hash2
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(committee4),
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
        assert_ok!(OnlineCommittee::submit_confirm_raw(Origin::signed(committee1), committee_upload_info.clone()));
        committee_upload_info.rand_str = "abcdefg2".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(Origin::signed(committee2), committee_upload_info.clone()));
        committee_upload_info.rand_str = "abcdefg3".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(Origin::signed(committee4), committee_upload_info.clone()));

        run_to_block(11);

        // stash申诉
        assert_ok!(OnlineCommittee::apply_slash_review(Origin::signed(controller), 0, vec![]));

        // TODO: add this
    })
}
