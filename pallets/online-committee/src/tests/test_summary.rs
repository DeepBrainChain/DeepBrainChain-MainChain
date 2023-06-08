use super::super::{mock::*, *};
use crate::tests::{committee1, committee2, committee3, committee4, stash};
use committee::CommitteeStakeInfo;
use dbc_support::{live_machine::LiveMachine, machine_type::CommitteeUploadInfo};
use frame_support::assert_ok;
use std::convert::TryInto;

fn get_machine_id() -> Vec<u8> {
    "484f457327950359de97c4b4c193bb3c8ddbe1dce56f038b3ac2b90e40995241"
        .as_bytes()
        .to_vec()
}

fn get_base_machine_info() -> CommitteeUploadInfo {
    CommitteeUploadInfo {
        machine_id: get_machine_id(),
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
        rand_str: "".as_bytes().to_vec(),
        is_support: true,
    }
}

// NOTE: 测试summary函数
// 当全部提交Hash+全部提交原始值时:
//
// case 1_1 只有1个提交信息，是支持
// case 1_2 只有1个提交信息，是反对
//
// case 2_1 2个提交信息，都支持，但内容相同
// case 2_2 2个提交信息，都支持，但内容不同
// case 2_3 2个提交信息，都反对，但内容不同
// case 2_4 2个提交信息，一支持一反对
//
// case 3_1: 3个支持，内容一致 ->上线
// case 3_2: 3个支持，2个内容一致，上线
// case 3_3: 3个支持，3个内容不一致，重新分派
// case 3_4: 2支持，1反对，支持信息一致。上线
// case 3_5: 2支持，1反对，支持信息不一致。重新分派
// case 3_6: 1支持，2反对，拒绝上线
// case 3_7: 3个反对，信息不一致 -> 拒绝上线

// case 1_1 只有1个提交信息，是支持
#[test]
fn test_summary_confirmation1_1() {
    new_test_with_init_params_ext().execute_with(|| {
        run_to_block(10);

        let upload_info1 = get_base_machine_info();

        let summary_expect1 = Summary {
            valid_vote: vec![*committee1],
            unruly: vec![*committee3, *committee2],
            info: Some(upload_info1.clone()),
            verify_result: VerifyResult::Confirmed,
            ..Default::default()
        };

        let machine_committee = OCMachineCommitteeList {
            book_time: 9,
            booked_committee: vec![*committee3, *committee2, *committee1],
            hashed_committee: vec![*committee3, *committee2, *committee1],
            confirm_start_time: 5432,
            confirmed_committee: vec![*committee1],
            onlined_committee: vec![],
            status: OCVerifyStatus::Summarizing,
        };
        let submit_info = vec![upload_info1];

        let summary = OnlineCommittee::summary_confirmation(machine_committee, submit_info);
        assert_eq!(summary_expect1, summary);
    })
}

// case 1_2 只有1个提交信息，是反对
#[test]
fn test_summary_confirmation1_2() {
    new_test_with_init_params_ext().execute_with(|| {
        run_to_block(10);

        let upload_info1 = CommitteeUploadInfo { is_support: false, ..get_base_machine_info() };

        let summary_expect1 = Summary {
            valid_vote: vec![*committee1],
            unruly: vec![*committee3, *committee2],
            verify_result: VerifyResult::Refused,
            ..Default::default()
        };

        let machine_committee = OCMachineCommitteeList {
            book_time: 9,
            booked_committee: vec![*committee3, *committee2, *committee1],
            hashed_committee: vec![*committee3, *committee2, *committee1],
            confirm_start_time: 5432,
            confirmed_committee: vec![*committee1],
            onlined_committee: vec![],
            status: OCVerifyStatus::Summarizing,
        };
        let submit_info = vec![upload_info1];

        let summary = OnlineCommittee::summary_confirmation(machine_committee, submit_info);
        assert_eq!(summary_expect1, summary);
    })
}

// case 2_1 2个提交信息，都支持，但内容相同
#[test]
fn test_summary_confirmation2_1() {
    new_test_with_init_params_ext().execute_with(|| {
        run_to_block(10);

        let upload_info2 = get_base_machine_info();
        let upload_info3 = get_base_machine_info();

        let summary_expect = Summary {
            valid_vote: vec![*committee3, *committee2],
            unruly: vec![*committee1],
            info: Some(upload_info2.clone()),
            verify_result: VerifyResult::Confirmed,
            ..Default::default()
        };

        let machine_committee = OCMachineCommitteeList {
            book_time: 9,
            booked_committee: vec![*committee3, *committee2, *committee1],
            hashed_committee: vec![*committee3, *committee2],
            confirm_start_time: 5432,
            confirmed_committee: vec![*committee3, *committee2],
            onlined_committee: vec![],
            status: OCVerifyStatus::Summarizing,
        };

        let submit_info = vec![upload_info3, upload_info2];

        let summary = OnlineCommittee::summary_confirmation(machine_committee, submit_info);
        assert_eq!(summary_expect, summary);
    })
}

// case 2_2 2个提交信息，都支持，但内容不同
#[test]
fn test_summary_confirmation2_2() {
    new_test_with_init_params_ext().execute_with(|| {
        run_to_block(10);

        let upload_info2 = get_base_machine_info();
        let upload_info3 = CommitteeUploadInfo { gpu_num: 8, ..get_base_machine_info() };

        let summary_expect = Summary {
            invalid_vote: vec![*committee3, *committee2],
            unruly: vec![*committee1],
            info: None,
            verify_result: VerifyResult::NoConsensus,
            ..Default::default()
        };

        let machine_committee = OCMachineCommitteeList {
            book_time: 9,
            booked_committee: vec![*committee3, *committee2, *committee1],
            hashed_committee: vec![*committee3, *committee2],
            confirm_start_time: 5432,
            confirmed_committee: vec![*committee3, *committee2],
            onlined_committee: vec![],
            status: OCVerifyStatus::Summarizing,
        };

        let submit_info = vec![upload_info3, upload_info2];

        let summary = OnlineCommittee::summary_confirmation(machine_committee, submit_info);
        assert_eq!(summary_expect, summary);
    })
}

// case 2_3 2个提交信息，都反对，但内容不同
#[test]
fn test_summary_confirmation2_3() {
    new_test_with_init_params_ext().execute_with(|| {
        run_to_block(10);

        let upload_info2 = CommitteeUploadInfo { is_support: false, ..get_base_machine_info() };
        let upload_info3 = upload_info2.clone();

        let summary_expect = Summary {
            unruly: vec![*committee1],
            valid_vote: vec![*committee3, *committee2],
            verify_result: VerifyResult::Refused,

            ..Default::default()
        };

        let machine_committee = OCMachineCommitteeList {
            book_time: 9,
            booked_committee: vec![*committee3, *committee2, *committee1],
            hashed_committee: vec![*committee3, *committee2],
            confirm_start_time: 5432,
            confirmed_committee: vec![*committee3, *committee2],
            onlined_committee: vec![],
            status: OCVerifyStatus::Summarizing,
        };
        let submit_info = vec![upload_info3, upload_info2];

        let summary = OnlineCommittee::summary_confirmation(machine_committee, submit_info);
        assert_eq!(summary_expect, summary);
    })
}

// case 2_4 2个提交信息，一支持一反对
#[test]
fn test_summary_confirmation2_4() {
    new_test_with_init_params_ext().execute_with(|| {
        run_to_block(10);

        let upload_info2 = CommitteeUploadInfo { is_support: false, ..get_base_machine_info() };
        let upload_info3 = get_base_machine_info();

        let summary_expect = Summary {
            unruly: vec![*committee1],
            invalid_vote: vec![*committee3, *committee2],
            verify_result: VerifyResult::NoConsensus,

            ..Default::default()
        };

        let machine_committee = OCMachineCommitteeList {
            book_time: 9,
            booked_committee: vec![*committee3, *committee2, *committee1],
            hashed_committee: vec![*committee3, *committee2],
            confirm_start_time: 5432,
            confirmed_committee: vec![*committee3, *committee2],
            onlined_committee: vec![],
            status: OCVerifyStatus::Summarizing,
        };
        let submit_info = vec![upload_info3, upload_info2];

        let summary = OnlineCommittee::summary_confirmation(machine_committee, submit_info);
        assert_eq!(summary_expect, summary);
    })
}

// case 3_1: 3个支持，内容一致 ->上线
#[test]
fn test_summary_confirmation3_1() {
    new_test_with_init_params_ext().execute_with(|| {
        run_to_block(10);

        let upload_info1 = get_base_machine_info();
        let upload_info2 = get_base_machine_info();
        let upload_info3 = get_base_machine_info();

        let summary_expect = Summary {
            valid_vote: vec![*committee3, *committee2, *committee1],
            info: Some(upload_info1.clone()),
            verify_result: VerifyResult::Confirmed,
            ..Default::default()
        };

        let machine_committee = OCMachineCommitteeList {
            book_time: 9,
            booked_committee: vec![*committee3, *committee2, *committee1],
            hashed_committee: vec![*committee3, *committee2, *committee1],
            confirm_start_time: 5432,
            confirmed_committee: vec![*committee3, *committee2, *committee1],
            onlined_committee: vec![],
            status: OCVerifyStatus::Summarizing,
        };
        let submit_info = vec![upload_info1, upload_info2, upload_info3];

        let summary = OnlineCommittee::summary_confirmation(machine_committee, submit_info);
        assert_eq!(summary_expect, summary);
    })
}

// case 3_2: 3个支持，2个内容一致，上线
#[test]
fn test_summary_confirmation3_2() {
    new_test_with_init_params_ext().execute_with(|| {
        run_to_block(10);

        let upload_info = get_base_machine_info();
        let upload_info2 = get_base_machine_info();
        let upload_info3 = CommitteeUploadInfo { gpu_num: 3, ..upload_info.clone() };

        let summary_expect = Summary {
            valid_vote: vec![*committee2, *committee1],
            invalid_vote: vec![*committee3],
            info: Some(upload_info.clone()),
            verify_result: VerifyResult::Confirmed,
            ..Default::default()
        };

        let machine_committee = OCMachineCommitteeList {
            book_time: 9,
            booked_committee: vec![*committee3, *committee2, *committee1],
            hashed_committee: vec![*committee3, *committee2, *committee1],
            confirm_start_time: 5432,
            confirmed_committee: vec![*committee3, *committee2, *committee1],
            onlined_committee: vec![],
            status: OCVerifyStatus::Summarizing,
        };
        let submit_info = vec![upload_info3, upload_info2, upload_info];

        let summary = OnlineCommittee::summary_confirmation(machine_committee, submit_info);
        assert_eq!(summary_expect, summary);
    })
}

// case 3_3: 3个支持，3个内容不一致，重新分派
#[test]
fn test_summary_confirmation3_3() {
    new_test_with_init_params_ext().execute_with(|| {
        run_to_block(10);

        let upload_info1 = get_base_machine_info();
        let upload_info2 = CommitteeUploadInfo { gpu_num: 5, ..upload_info1.clone() };
        let upload_info3 = CommitteeUploadInfo { gpu_num: 3, ..upload_info1.clone() };

        let summary_expect = Summary {
            invalid_vote: vec![*committee3, *committee2, *committee1],
            verify_result: VerifyResult::NoConsensus,
            ..Default::default()
        };

        let machine_committee = OCMachineCommitteeList {
            book_time: 9,
            booked_committee: vec![*committee3, *committee2, *committee1],
            hashed_committee: vec![*committee3, *committee2, *committee1],
            confirm_start_time: 5432,
            confirmed_committee: vec![*committee3, *committee2, *committee1],
            onlined_committee: vec![],
            status: OCVerifyStatus::Summarizing,
        };
        let submit_info = vec![upload_info3, upload_info2, upload_info1];

        let summary = OnlineCommittee::summary_confirmation(machine_committee, submit_info);
        assert_eq!(summary_expect, summary);
    })
}

// case 3_4: 2支持，1反对，支持信息一致。上线
#[test]
fn test_summary_confirmation3_4() {
    new_test_with_init_params_ext().execute_with(|| {
        run_to_block(10);
        let upload_info = get_base_machine_info();
        let upload_info2 = get_base_machine_info();
        let upload_info3 = CommitteeUploadInfo { is_support: false, ..upload_info.clone() };

        let summary_expect = Summary {
            valid_vote: vec![*committee2, *committee1],
            invalid_vote: vec![*committee3],
            info: Some(upload_info.clone()),
            verify_result: VerifyResult::Confirmed,
            ..Default::default()
        };

        let machine_committee = OCMachineCommitteeList {
            book_time: 9,
            booked_committee: vec![*committee3, *committee2, *committee1],
            hashed_committee: vec![*committee3, *committee2, *committee1],
            confirm_start_time: 5432,
            confirmed_committee: vec![*committee3, *committee2, *committee1],
            onlined_committee: vec![],
            status: OCVerifyStatus::Summarizing,
        };
        let submit_info = vec![upload_info3, upload_info2, upload_info];

        let summary = OnlineCommittee::summary_confirmation(machine_committee, submit_info);
        assert_eq!(summary_expect, summary);
    })
}

// case 3_5: 2支持，1反对，支持信息不一致。重新分派
#[test]
fn test_summary_confirmation3_5() {
    new_test_with_init_params_ext().execute_with(|| {
        run_to_block(10);

        let upload_info1 = get_base_machine_info();
        let upload_info2 = CommitteeUploadInfo { gpu_num: 3, ..upload_info1.clone() };
        let upload_info3 = CommitteeUploadInfo { is_support: false, ..upload_info1.clone() };

        let summary_expect = Summary {
            invalid_vote: vec![*committee3, *committee2, *committee1],
            verify_result: VerifyResult::NoConsensus,
            ..Default::default()
        };

        let machine_committee = OCMachineCommitteeList {
            book_time: 9,
            booked_committee: vec![*committee3, *committee2, *committee1],
            hashed_committee: vec![*committee3, *committee2, *committee1],
            confirm_start_time: 5432,
            confirmed_committee: vec![*committee3, *committee2, *committee1],
            onlined_committee: vec![],
            status: OCVerifyStatus::Summarizing,
        };
        let submit_info = vec![upload_info3, upload_info2, upload_info1];

        let summary = OnlineCommittee::summary_confirmation(machine_committee, submit_info);
        assert_eq!(summary_expect, summary);
    })
}

// case 3_6: 1支持，2反对，拒绝上线
#[test]
fn test_summary_confirmation3_6() {
    new_test_with_init_params_ext().execute_with(|| {
        run_to_block(10);

        let upload_info1 = CommitteeUploadInfo { is_support: false, ..get_base_machine_info() };
        let upload_info2 = CommitteeUploadInfo { ..get_base_machine_info() };
        let upload_info3 = CommitteeUploadInfo { is_support: false, ..get_base_machine_info() };

        let summary_expect = Summary {
            valid_vote: vec![*committee3, *committee1],
            invalid_vote: vec![*committee2],
            verify_result: VerifyResult::Refused,
            ..Default::default()
        };

        let machine_committee = OCMachineCommitteeList {
            book_time: 9,
            booked_committee: vec![*committee3, *committee2, *committee1],
            hashed_committee: vec![*committee3, *committee2, *committee1],
            confirm_start_time: 5432,
            confirmed_committee: vec![*committee3, *committee2, *committee1],
            onlined_committee: vec![],
            status: OCVerifyStatus::Summarizing,
        };
        let submit_info = vec![upload_info3, upload_info2, upload_info1];

        let summary = OnlineCommittee::summary_confirmation(machine_committee, submit_info);
        assert_eq!(summary_expect, summary);
    })
}

// case 3_7: 3个反对，信息不一致 -> 拒绝上线
#[test]
fn test_summary_confirmation3_7() {
    new_test_with_init_params_ext().execute_with(|| {
        run_to_block(10);

        let upload_info1 =
            CommitteeUploadInfo { gpu_num: 2, is_support: false, ..get_base_machine_info() };
        let upload_info2 = CommitteeUploadInfo { gpu_num: 3, ..upload_info1.clone() };
        let upload_info3 = CommitteeUploadInfo { gpu_num: 4, ..upload_info1.clone() };

        let summary_expect = Summary {
            valid_vote: vec![*committee3, *committee2, *committee1],
            verify_result: VerifyResult::Refused,
            ..Default::default()
        };

        let machine_committee = OCMachineCommitteeList {
            book_time: 9,
            booked_committee: vec![*committee3, *committee2, *committee1],
            hashed_committee: vec![*committee3, *committee2, *committee1],
            confirm_start_time: 5432,
            confirmed_committee: vec![*committee3, *committee2, *committee1],
            onlined_committee: vec![],
            status: OCVerifyStatus::Summarizing,
        };
        let submit_info = vec![upload_info1, upload_info2, upload_info3];

        let summary = OnlineCommittee::summary_confirmation(machine_committee, submit_info);
        assert_eq!(summary_expect, summary);
    })
}

fn decode_box_pubkey<T: AsRef<[u8]>>(x: T) -> [u8; 32] {
    hex::decode(x).unwrap().try_into().unwrap()
}

// 机器成功上线，拒绝的委员会惩罚被执行，
// 且惩罚执行时正确退还未被惩罚的委员会的质押
#[test]
fn test_machine_online_succeed_slash_execed() {
    new_test_with_online_machine_distribution().execute_with(|| {
        let committee1_box_pubkey =
            decode_box_pubkey("ff3033c763f71bc51f372c1dc5095accc26880e138df84cac13c46bfd7dbd74f");
        let committee2_box_pubkey =
            decode_box_pubkey("336404f7d316565cc3c3350e70561f4177803e0bb02a7f2e4e02a4f0e361157e");
        let committee3_box_pubkey =
            decode_box_pubkey("a7804e30caa5645e97489b2d4711e3d8f4e17a683338cba97a53b960648f0438");
        let committee4_box_pubkey =
            decode_box_pubkey("5eec53877f4b18c8b003fa983d27ef2e5518b7e4d08d482922a7787f2ea75529");

        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();
        // let mut committee_upload_info = get_base_machine_info();
        // committee_upload_info.rand_str = "abcdefg1".as_bytes().to_vec();

        // 三个委员会提交Hash
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
            is_support: true,
        };

        // 委员会提交机器Hash
        let machine_info_hash1: [u8; 16] =
            hex::decode("fd8885a22a9d9784adaa36effcd77522").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            RuntimeOrigin::signed(*committee1),
            machine_id.clone(),
            machine_info_hash1
        ));
        let machine_info_hash2: [u8; 16] =
            hex::decode("c016090e0943c17f5d4999dc6eb52683").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            RuntimeOrigin::signed(*committee3),
            machine_id.clone(),
            machine_info_hash2
        ));

        let machine_info_hash3: [u8; 16] =
            hex::decode("fe3d8c7eb5dc36f3f923aff6f3367544").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            RuntimeOrigin::signed(*committee4),
            machine_id.clone(),
            machine_info_hash3
        ));

        // 委员会提交原始信息
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            RuntimeOrigin::signed(*committee1),
            committee_upload_info.clone()
        ));
        committee_upload_info.rand_str = "abcdefg2".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            RuntimeOrigin::signed(*committee3),
            committee_upload_info.clone()
        ));

        // 第三个委员会提交错误的机器信息
        committee_upload_info.rand_str = "abcdefg3".as_bytes().to_vec();
        committee_upload_info.mem_num = 450;
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            RuntimeOrigin::signed(*committee4),
            committee_upload_info.clone()
        ));

        run_to_block(11);
        let current_machine_info = OnlineProfile::machines_info(&machine_id).unwrap();
        assert_eq!(
            OnlineProfile::live_machines(),
            LiveMachine { online_machine: vec![machine_id.clone()], ..Default::default() }
        );
        assert_eq!(current_machine_info.reward_committee, vec![*committee3, *committee1]);

        assert_eq!(
            OnlineCommittee::pending_slash(0),
            Some(crate::OCPendingSlashInfo {
                machine_id,
                stash_slash_amount: 0,
                inconsistent_committee: vec![*committee4],
                reward_committee: vec![*committee3, *committee1],
                committee_stake: 1000 * ONE_DBC,
                slash_time: 11,
                slash_exec_time: 11 + 2880 * 2,
                book_result: crate::OCBookResultType::OnlineSucceed,
                slash_result: crate::OCSlashResult::Pending,
                machine_stash: None,
                unruly_committee: vec![]
            })
        );
        assert_ok!(OnlineCommittee::unhandled_slash().binary_search(&0));

        // 检查三个委员会的质押
        assert_eq!(
            Committee::committee_stake(&*committee1),
            committee::CommitteeStakeInfo {
                box_pubkey: committee1_box_pubkey,
                staked_amount: 20000 * ONE_DBC,
                used_stake: 1000 * ONE_DBC,
                can_claim_reward: 0,
                claimed_reward: 0,
            }
        );
        assert_eq!(
            Committee::committee_stake(&*committee2),
            committee::CommitteeStakeInfo {
                box_pubkey: committee2_box_pubkey,
                staked_amount: 20000 * ONE_DBC,
                used_stake: 0,
                can_claim_reward: 0,
                claimed_reward: 0,
            }
        );
        assert_eq!(
            Committee::committee_stake(&*committee3),
            committee::CommitteeStakeInfo {
                box_pubkey: committee3_box_pubkey,
                staked_amount: 20000 * ONE_DBC,
                used_stake: 1000 * ONE_DBC,
                can_claim_reward: 0,
                claimed_reward: 0,
            }
        );
        assert_eq!(
            Committee::committee_stake(&*committee4),
            committee::CommitteeStakeInfo {
                box_pubkey: committee4_box_pubkey,
                staked_amount: 20000 * ONE_DBC,
                used_stake: 1000 * ONE_DBC,
                can_claim_reward: 0,
                claimed_reward: 0,
            }
        );

        assert_eq!(Balances::reserved_balance(&*committee1), 20000 * ONE_DBC);
        assert_eq!(Balances::reserved_balance(&*committee2), 20000 * ONE_DBC);
        assert_eq!(Balances::reserved_balance(&*committee3), 20000 * ONE_DBC);
        assert_eq!(Balances::reserved_balance(&*committee4), 20000 * ONE_DBC);

        // 测试执行惩罚
        run_to_block(12 + 2880 * 2);

        // 检查三个委员会的质押
        assert_eq!(
            Committee::committee_stake(&*committee1),
            committee::CommitteeStakeInfo {
                box_pubkey: committee1_box_pubkey,
                staked_amount: 20000 * ONE_DBC,
                used_stake: 0,
                can_claim_reward: 1375 * ONE_DBC, // 1100000 * 0.25 * 0.01 / 2
                claimed_reward: 0,
            }
        );
        assert_eq!(
            Committee::committee_stake(&*committee2),
            committee::CommitteeStakeInfo {
                box_pubkey: committee2_box_pubkey,
                staked_amount: 20000 * ONE_DBC,
                used_stake: 0,
                can_claim_reward: 0,
                claimed_reward: 0,
            }
        );
        assert_eq!(
            Committee::committee_stake(&*committee3),
            committee::CommitteeStakeInfo {
                box_pubkey: committee3_box_pubkey,
                staked_amount: 20000 * ONE_DBC,
                used_stake: 0,
                can_claim_reward: 1375 * ONE_DBC,
                claimed_reward: 0,
            }
        );
        assert_eq!(
            Committee::committee_stake(&*committee4),
            committee::CommitteeStakeInfo {
                box_pubkey: committee4_box_pubkey,
                staked_amount: 19000 * ONE_DBC,
                used_stake: 0,
                can_claim_reward: 0,
                claimed_reward: 0,
            }
        );

        assert_eq!(Balances::reserved_balance(&*committee1), 20000 * ONE_DBC);
        assert_eq!(Balances::reserved_balance(&*committee2), 20000 * ONE_DBC);
        assert_eq!(Balances::reserved_balance(&*committee3), 20000 * ONE_DBC);
        assert_eq!(Balances::reserved_balance(&*committee4), 19000 * ONE_DBC);

        assert!(OnlineCommittee::unhandled_slash().binary_search(&0).is_err());
    })
}

// 机器上线失败，支持的委员会惩罚被执行
#[test]
fn test_machine_online_failed_slash_execed() {
    new_test_with_online_machine_distribution().execute_with(|| {
        let committee1_box_pubkey =
            decode_box_pubkey("ff3033c763f71bc51f372c1dc5095accc26880e138df84cac13c46bfd7dbd74f");
        let _committee2_box_pubkey =
            decode_box_pubkey("336404f7d316565cc3c3350e70561f4177803e0bb02a7f2e4e02a4f0e361157e");
        let committee3_box_pubkey =
            decode_box_pubkey("a7804e30caa5645e97489b2d4711e3d8f4e17a683338cba97a53b960648f0438");
        let committee4_box_pubkey =
            decode_box_pubkey("5eec53877f4b18c8b003fa983d27ef2e5518b7e4d08d482922a7787f2ea75529");

        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();

        // 三个委员会提交Hash
        let base_info = get_base_machine_info();
        let mut committee_upload_info = CommitteeUploadInfo {
            machine_id: machine_id.clone(),

            rand_str: "abcdefg1".as_bytes().to_vec(),
            is_support: false,
            ..base_info
        };

        // 委员会提交机器Hash
        // 委员会1，2反对，3支持
        let machine_info_hash1: [u8; 16] =
            hex::decode("cee14a520ba6a988c306aab9dc3794b1").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            RuntimeOrigin::signed(*committee1),
            machine_id.clone(),
            machine_info_hash1
        ));
        let machine_info_hash2: [u8; 16] =
            hex::decode("8c7e7ca563169689f1c789f8d4f510f8").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            RuntimeOrigin::signed(*committee3),
            machine_id.clone(),
            machine_info_hash2
        ));

        let machine_info_hash3: [u8; 16] =
            hex::decode("4a6b2df1e1a77b9bcdab5e31dc7950d2").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            RuntimeOrigin::signed(*committee4),
            machine_id.clone(),
            machine_info_hash3
        ));

        // 委员会提交原始信息
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            RuntimeOrigin::signed(*committee1),
            committee_upload_info.clone()
        ));
        committee_upload_info.rand_str = "abcdefg2".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            RuntimeOrigin::signed(*committee3),
            committee_upload_info.clone()
        ));

        // 第三个委员会提交错误的机器信息
        committee_upload_info.rand_str = "abcdefg3".as_bytes().to_vec();
        committee_upload_info.is_support = true;
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            RuntimeOrigin::signed(*committee4),
            committee_upload_info
        ));

        run_to_block(11);
        // 机器被拒绝上线，将会产生对委员会3和satsh账户的pending_slash
        assert_eq!(
            OnlineCommittee::pending_slash(0),
            Some(crate::OCPendingSlashInfo {
                machine_id: machine_id.clone(),
                machine_stash: Some(*stash),
                stash_slash_amount: 5000 * ONE_DBC, // 10,0000 * 5 / 100

                inconsistent_committee: vec![*committee4],
                unruly_committee: vec![],
                reward_committee: vec![*committee3, *committee1],
                committee_stake: 1000 * ONE_DBC,

                slash_time: 11,
                slash_exec_time: 11 + 2880 * 2,

                book_result: crate::OCBookResultType::OnlineRefused,
                slash_result: crate::OCSlashResult::Pending,
            })
        );
        assert_ok!(OnlineCommittee::unhandled_slash().binary_search(&0));

        assert_eq!(
            OnlineProfile::live_machines(),
            LiveMachine { refused_machine: vec![machine_id], ..Default::default() }
        );

        // 检查前后委员会/stash质押的变化
        assert_eq!(
            Committee::committee_stake(&*committee1),
            committee::CommitteeStakeInfo {
                box_pubkey: committee1_box_pubkey,
                staked_amount: 20000 * ONE_DBC,
                used_stake: 1000 * ONE_DBC,
                can_claim_reward: 0,
                claimed_reward: 0,
            }
        );
        assert_eq!(
            Committee::committee_stake(&*committee3),
            committee::CommitteeStakeInfo {
                box_pubkey: committee3_box_pubkey,
                staked_amount: 20000 * ONE_DBC,
                used_stake: 1000 * ONE_DBC,
                can_claim_reward: 0,
                claimed_reward: 0,
            }
        );
        assert_eq!(
            Committee::committee_stake(&*committee4),
            committee::CommitteeStakeInfo {
                box_pubkey: committee4_box_pubkey,
                staked_amount: 20000 * ONE_DBC,
                used_stake: 1000 * ONE_DBC,
                can_claim_reward: 0,
                claimed_reward: 0,
            }
        );

        assert_eq!(Balances::reserved_balance(&*committee1), 20000 * ONE_DBC);
        assert_eq!(Balances::reserved_balance(&*committee3), 20000 * ONE_DBC);
        assert_eq!(Balances::reserved_balance(&*committee4), 20000 * ONE_DBC);
        assert_eq!(Balances::reserved_balance(&*stash), 5000 * ONE_DBC);
        assert_eq!(Balances::free_balance(&*stash), (10000000 - 5000) * ONE_DBC);

        // 测试执行惩罚
        run_to_block(12 + 2880 * 2);
        assert_eq!(
            Committee::committee_stake(&*committee1),
            committee::CommitteeStakeInfo {
                box_pubkey: committee1_box_pubkey,
                staked_amount: 20000 * ONE_DBC,
                used_stake: 0,
                can_claim_reward: 0, // 1100000 * 0.25 * 0.01 / 2
                claimed_reward: 0,
            }
        );
        assert_eq!(
            Committee::committee_stake(&*committee3),
            committee::CommitteeStakeInfo {
                box_pubkey: committee3_box_pubkey,
                staked_amount: 20000 * ONE_DBC,
                used_stake: 0,
                can_claim_reward: 0,
                claimed_reward: 0,
            }
        );
        assert_eq!(
            Committee::committee_stake(&*committee4),
            committee::CommitteeStakeInfo {
                box_pubkey: committee4_box_pubkey,
                staked_amount: 19000 * ONE_DBC,
                used_stake: 0,
                can_claim_reward: 0,
                claimed_reward: 0,
            }
        );

        assert_eq!(Balances::reserved_balance(&*committee1), 20000 * ONE_DBC);
        assert_eq!(Balances::reserved_balance(&*committee3), 20000 * ONE_DBC);
        assert_eq!(Balances::reserved_balance(&*committee4), 19000 * ONE_DBC);
        assert_eq!(Balances::reserved_balance(&*stash), 0);
        assert_eq!(Balances::free_balance(&*stash), (10000000 - 5000) * ONE_DBC);

        assert!(OnlineCommittee::unhandled_slash().binary_search(&0).is_err());
    })
}

// 机器成功上线，反对上线/上不同信息的委员会被惩罚的申述
#[test]
fn test_machine_online_succeed_against_committee_apply_review() {
    new_test_with_online_machine_distribution().execute_with(|| {
        // let committee1_box_pubkey =
        // hex::decode("ff3033c763f71bc51f372c1dc5095accc26880e138df84cac13c46bfd7dbd74f")
        //     .unwrap()
        //     .try_into()
        //     .unwrap();
        // let committee2_box_pubkey =
        // hex::decode("336404f7d316565cc3c3350e70561f4177803e0bb02a7f2e4e02a4f0e361157e")
        //     .unwrap()
        //     .try_into()
        //     .unwrap();
        // let committee3_box_pubkey =
        // hex::decode("a7804e30caa5645e97489b2d4711e3d8f4e17a683338cba97a53b960648f0438")
        //     .unwrap()
        //     .try_into()
        //     .unwrap();
        // let committee4_box_pubkey =
        // hex::decode("5eec53877f4b18c8b003fa983d27ef2e5518b7e4d08d482922a7787f2ea75529")
        //     .unwrap()
        //     .try_into()
        //     .unwrap();

        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();

        // 三个委员会提交Hash
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
            is_support: true,
        };

        // 委员会提交机器Hash
        let machine_info_hash1: [u8; 16] =
            hex::decode("fd8885a22a9d9784adaa36effcd77522").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            RuntimeOrigin::signed(*committee1),
            machine_id.clone(),
            machine_info_hash1
        ));
        let machine_info_hash2: [u8; 16] =
            hex::decode("c016090e0943c17f5d4999dc6eb52683").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            RuntimeOrigin::signed(*committee3),
            machine_id.clone(),
            machine_info_hash2
        ));

        let machine_info_hash3: [u8; 16] =
            hex::decode("fe3d8c7eb5dc36f3f923aff6f3367544").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            RuntimeOrigin::signed(*committee4),
            machine_id,
            machine_info_hash3
        ));

        // 委员会提交原始信息
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            RuntimeOrigin::signed(*committee1),
            committee_upload_info.clone()
        ));
        committee_upload_info.rand_str = "abcdefg2".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            RuntimeOrigin::signed(*committee3),
            committee_upload_info.clone()
        ));

        // 第三个委员会提交错误的机器信息
        committee_upload_info.rand_str = "abcdefg3".as_bytes().to_vec();
        committee_upload_info.mem_num = 450;
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            RuntimeOrigin::signed(*committee4),
            committee_upload_info
        ));

        run_to_block(12);

        // committee 3 apply_slash_review
        let slash_reason = "They are wrong.".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::apply_slash_review(
            RuntimeOrigin::signed(*committee4),
            0,
            slash_reason.clone()
        ));

        assert_eq!(
            OnlineCommittee::pending_slash_review(0),
            Some(crate::OCPendingSlashReviewInfo {
                applicant: *committee4,
                staked_amount: 1000 * ONE_DBC,
                apply_time: 13,
                expire_time: 11 + 2880 * 2,
                reason: slash_reason,
            })
        );
        // 检查commttee reserve
        assert_eq!(Balances::reserved_balance(&*committee4), (20000 + 1000) * ONE_DBC);

        assert_ok!(OnlineCommittee::do_cancel_slash(0));
    })
}

// 两个委员会提交信息不同，另一委员会未完成验证，则无法上线。且产生惩罚
#[test]
fn test_machine_noconsensus_works() {
    new_test_with_online_machine_distribution().execute_with(|| {
        let base_info = get_base_machine_info();

        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();

        let hash1 = hex::decode("80d57cfbe2e8a56c889ec220ad204b4a").unwrap().try_into().unwrap();
        let upload_info1 = CommitteeUploadInfo {
            machine_id: machine_id.clone(),
            rand_str: "abc1".as_bytes().to_vec(),
            ..base_info.clone()
        };
        let hash2 = hex::decode("f2e10bdd510e642a127145a8abbd8214").unwrap().try_into().unwrap();
        let upload_info2 = CommitteeUploadInfo {
            machine_id: machine_id.clone(),
            rand_str: "abc2".as_bytes().to_vec(),
            gpu_num: 5,
            ..base_info.clone()
        };

        assert_ok!(OnlineCommittee::submit_confirm_hash(
            RuntimeOrigin::signed(*committee1),
            machine_id.clone(),
            hash1
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            RuntimeOrigin::signed(*committee3),
            machine_id.clone(),
            hash2
        ));

        // 无共识，机器将重新分派，并惩罚未完成工作的委员会
        run_to_block(11 + 2880 + 1440); // 4331

        // 委员会提交原始信息
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            RuntimeOrigin::signed(*committee1),
            upload_info1
        ));
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            RuntimeOrigin::signed(*committee3),
            upload_info2
        ));

        // 这个块高时，既发放奖励，又重新分派
        run_to_block(11 + 2880 + 1440 + 1);

        assert_eq!(
            OnlineCommittee::pending_slash(0),
            Some(crate::OCPendingSlashInfo {
                machine_id: machine_id.clone(),
                inconsistent_committee: vec![*committee3, *committee1],
                unruly_committee: vec![*committee4],
                reward_committee: vec![],
                committee_stake: 1000 * ONE_DBC,
                slash_time: 4332,
                slash_exec_time: 4332 + 2880 * 2,
                book_result: crate::OCBookResultType::NoConsensus,
                slash_result: crate::OCSlashResult::Pending,
                machine_stash: None,
                stash_slash_amount: 0
            })
        );

        assert_eq!(OnlineCommittee::machine_committee(&machine_id), OCMachineCommitteeList {
            book_time: 4332,
            booked_committee: vec![*committee3,*committee2, *committee4],
            hashed_committee: vec![],
            confirm_start_time:8652,
            confirmed_committee: vec![],
            onlined_committee: vec![],
            status: OCVerifyStatus::default(),
        });

        assert_eq!(
            &CommitteeStakeInfo {
                box_pubkey: Default::default(),
                ..Committee::committee_stake(&*committee1)
            },
            &committee::CommitteeStakeInfo {
                staked_amount: 20000 * ONE_DBC,
                used_stake: 0 * ONE_DBC ,             // 没有重新分派给committee1
                can_claim_reward: 0,        // 1100000 * 0.25 * 0.01 / 2
                claimed_reward: 0,
                ..Default::default()
            }
        );

        assert_eq!(
            &CommitteeStakeInfo {
                box_pubkey: Default::default(),
                ..Committee::committee_stake(&*committee2)
            },
            &committee::CommitteeStakeInfo {
                staked_amount: 20000 * ONE_DBC,
                used_stake: 1000 * ONE_DBC ,             // 重新分派给committee2
                can_claim_reward: 0,        // 1100000 * 0.25 * 0.01 / 2
                claimed_reward: 0,
                ..Default::default()
            }
        );

        assert_eq!(
            &CommitteeStakeInfo {
                box_pubkey: Default::default(),
                ..Committee::committee_stake(&*committee3)
            },
            &committee::CommitteeStakeInfo {
                staked_amount: 20000 * ONE_DBC,
                used_stake: 1000 * ONE_DBC ,             // 重新分派给committee3
                can_claim_reward: 0,        // 1100000 * 0.25 * 0.01 / 2
                claimed_reward: 0,
                ..Default::default()
            }
        );

        assert_eq!(
            &CommitteeStakeInfo {
                box_pubkey: Default::default(),
                ..Committee::committee_stake(&*committee4)
            },
            &committee::CommitteeStakeInfo {
                staked_amount: 20000 * ONE_DBC,
                used_stake: 2000 * ONE_DBC ,             // 重新分派给committee4
                can_claim_reward: 0,        // 1100000 * 0.25 * 0.01 / 2
                claimed_reward: 0,
                ..Default::default()
            }
        );

        assert_eq!(Balances::reserved_balance(&*committee1), 20000 * ONE_DBC);
        assert_eq!(Balances::reserved_balance(&*committee2), 20000 * ONE_DBC);
        assert_eq!(Balances::reserved_balance(&*committee3), 20000 * ONE_DBC);
        assert_eq!(Balances::reserved_balance(&*committee4), 20000 * ONE_DBC); // 惩罚还未执行
    })
}
