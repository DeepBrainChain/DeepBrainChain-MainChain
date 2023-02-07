use super::super::{mock::*, *};
use crate::tests::{committee1, committee2, committee3, committee4, stash};
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

// fn decode_hash(hash: &str) -> [u8; 16] {
//     hex::decode(hash).unwrap().try_into().unwrap()
// }

// fn get_support_info() {
//     let rand_str1 = "abcdefg1";
//     let rand_str2 = "abcdefg2";
//     let rand_str3 = "abcdefg3";

//     let hash1 = decode_hash("2a0834c7aa168781cd2c40bc5259833e");
//     let hash2 = decode_hash("422b76afb204fc7b94afe2912d82c659");
//     let hash3 = decode_hash("08e1544c321b862db7f09008551a022f");
// }

// fn get_against_info() {
//     let rand_str1 = "abcdefg1";
//     let rand_str2 = "abcdefg2";
//     let rand_str3 = "abcdefg3";

//     let hash1 = decode_hash("9e100b5d89fdc4dc0932bfda23474f08");
//     let hash2 = decode_hash("8702590323bf06ffb5f0fc5d1f9e0770");
//     let hash3 = decode_hash("cf838f58d88f5ed2b66548531e4e0ca4");
// }

// NOTE: 测试summary函数
// 当全部提交Hash+全部提交原始值时:
// case 1: 3个支持，内容一致 ->上线
// case 2: 3个支持，2内容一致 -> 上线 + 惩罚
// case 3: 2个支持，1个反对 (2个一致) -> 上线 + 惩罚
// case 4: 3个支持，内容都不一致 -> 无共识 + 重新分配
// case 5: 2个支持，1个反对（2个不一致） -> 无共识 + 重新分配
// case 6: 2个反对，1个支持 -> 不上线 + 奖励 + 惩罚
// case 7: 3个反对 -> 不上线 + 奖励
// case 8: 2提交Hash， 2提交原始值，都是反对
// case 9: 2提交Hash， 2提交原始值，都是支持
// case 10: 全部提交Hash，2提交原始值，且都是支持，两个不相同
// case 11: 全部提交Hash，2提交原始值时，且都是支持，两个相同

// case 1: 3个支持，内容一致 ->上线
#[test]
fn test_summary_confirmation1() {
    new_test_with_init_params_ext().execute_with(|| {
        run_to_block(10);

        let machine_id = get_machine_id();
        // let mut upload_info = get_base_machine_info();

        // 构建 machine_committee
        <MachineCommittee<TestRuntime>>::insert(
            &machine_id,
            OCMachineCommitteeList {
                book_time: 9,
                booked_committee: vec![*committee3, *committee2, *committee1],
                hashed_committee: vec![*committee3, *committee2, *committee1],
                confirm_start_time: 5432,
                confirmed_committee: vec![*committee3, *committee2, *committee1],
                onlined_committee: vec![],
                status: OCVerifyStatus::Summarizing,
            },
        );

        let machine_info_hash: [u8; 16] =
            hex::decode("d80b116fd318f19fd89da792aba5e875").unwrap().try_into().unwrap();

        let committee_upload_info = get_base_machine_info();

        let committee_ops = OCCommitteeOps {
            staked_dbc: 1000 * ONE_DBC,
            verify_time: vec![],
            confirm_hash: machine_info_hash,
            hash_time: 16887,
            confirm_time: 16891,
            machine_status: OCMachineStatus::Confirmed,
            machine_info: committee_upload_info,
        };

        let committee1_ops = OCCommitteeOps {
            verify_time: vec![1622, 3062, 4502],
            confirm_hash: machine_info_hash,
            ..committee_ops.clone()
        };

        let committee2_ops = OCCommitteeOps {
            verify_time: vec![1142, 2582, 4022],
            confirm_hash: machine_info_hash,
            ..committee_ops.clone()
        };

        let committee3_ops = OCCommitteeOps {
            verify_time: vec![662, 2102, 3542],
            confirm_hash: machine_info_hash,
            ..committee_ops.clone()
        };

        // 构建committee_ops
        <CommitteeOps<TestRuntime>>::insert(&*committee1, &machine_id, committee1_ops);
        <CommitteeOps<TestRuntime>>::insert(&*committee2, &machine_id, committee2_ops);
        <CommitteeOps<TestRuntime>>::insert(&*committee3, &machine_id, committee3_ops);

        let summary = Summary {
            valid_vote: vec![*committee3, *committee2, *committee1],
            info: Some(committee_ops.machine_info),
            verify_result: VerifyResult::Confirmed,
            ..Default::default()
        };

        assert_eq!(OnlineCommittee::summary_confirmation2(&machine_id), summary);
    })
}

// case 2: 3个支持，2内容一致 -> 上线 + 惩罚
#[test]
fn test_summary_confirmation2() {
    new_test_with_init_params_ext().execute_with(|| {
        run_to_block(10);

        let machine_id = get_machine_id();

        // 构建 machine_committee
        <MachineCommittee<TestRuntime>>::insert(
            &machine_id,
            OCMachineCommitteeList {
                book_time: 9,
                booked_committee: vec![*committee3, *committee2, *committee1],
                hashed_committee: vec![*committee3, *committee2, *committee1],
                confirm_start_time: 5432,
                confirmed_committee: vec![*committee3, *committee2, *committee1],
                onlined_committee: vec![],
                status: OCVerifyStatus::Summarizing,
            },
        );

        let machine_info_hash: [u8; 16] =
            hex::decode("d80b116fd318f19fd89da792aba5e875").unwrap().try_into().unwrap();
        let committee_upload_info = get_base_machine_info();

        let committee_ops = OCCommitteeOps {
            staked_dbc: 1000 * ONE_DBC,
            verify_time: vec![],
            confirm_hash: machine_info_hash,
            hash_time: 16887,
            confirm_time: 16891,
            machine_status: OCMachineStatus::Confirmed,
            machine_info: committee_upload_info,
        };

        let committee1_ops = OCCommitteeOps {
            verify_time: vec![1622, 3062, 4502],
            confirm_hash: machine_info_hash,
            ..committee_ops.clone()
        };

        let committee2_ops = OCCommitteeOps {
            verify_time: vec![1142, 2582, 4022],
            confirm_hash: machine_info_hash,
            ..committee_ops.clone()
        };

        let committee3_ops = OCCommitteeOps {
            verify_time: vec![662, 2102, 3542],
            confirm_hash: machine_info_hash,
            machine_info: CommitteeUploadInfo { gpu_num: 3, ..committee_ops.machine_info.clone() },
            ..committee_ops.clone()
        };

        // 构建committee_ops
        <CommitteeOps<TestRuntime>>::insert(&*committee1, &machine_id, committee1_ops);
        <CommitteeOps<TestRuntime>>::insert(&*committee2, &machine_id, committee2_ops);
        <CommitteeOps<TestRuntime>>::insert(&*committee3, &machine_id, committee3_ops);

        let summary = Summary {
            valid_vote: vec![*committee2, *committee1],
            invalid_vote: vec![*committee3],
            info: Some(committee_ops.machine_info),
            verify_result: VerifyResult::Confirmed,
            ..Default::default()
        };

        assert_eq!(OnlineCommittee::summary_confirmation2(&machine_id), summary);
    })
}

// case 3: 2个支持，1个反对 (2个一致) -> 上线 + 惩罚
#[test]
fn test_summary_confirmation3() {
    new_test_with_init_params_ext().execute_with(|| {
        run_to_block(10);
        let machine_id = get_machine_id();

        // 构建 machine_committee
        <MachineCommittee<TestRuntime>>::insert(
            &machine_id,
            OCMachineCommitteeList {
                book_time: 9,
                booked_committee: vec![*committee3, *committee2, *committee1],
                hashed_committee: vec![*committee3, *committee2, *committee1],
                confirm_start_time: 5432,
                confirmed_committee: vec![*committee3, *committee2, *committee1],
                onlined_committee: vec![],
                status: OCVerifyStatus::Summarizing,
            },
        );

        let machine_info_hash: [u8; 16] =
            hex::decode("d80b116fd318f19fd89da792aba5e875").unwrap().try_into().unwrap();
        let committee_upload_info = get_base_machine_info();

        let committee_ops = OCCommitteeOps {
            staked_dbc: 1000 * ONE_DBC,
            verify_time: vec![],
            confirm_hash: machine_info_hash,
            hash_time: 16888,
            confirm_time: 16891,
            machine_status: OCMachineStatus::Confirmed,
            machine_info: committee_upload_info,
        };

        let committee1_ops = OCCommitteeOps {
            verify_time: vec![1622, 3062, 4502],
            confirm_hash: machine_info_hash,
            ..committee_ops.clone()
        };

        let committee2_ops = OCCommitteeOps {
            verify_time: vec![1142, 2582, 4022],
            confirm_hash: machine_info_hash,
            ..committee_ops.clone()
        };

        let committee3_ops = OCCommitteeOps {
            verify_time: vec![662, 2102, 3542],
            confirm_hash: machine_info_hash,
            machine_info: CommitteeUploadInfo {
                gpu_num: 3,
                is_support: false,
                ..committee_ops.machine_info.clone()
            },
            ..committee_ops.clone()
        };

        // 构建committee_ops
        CommitteeOps::<TestRuntime>::insert(&*committee1, &machine_id, committee1_ops);
        CommitteeOps::<TestRuntime>::insert(&*committee2, &machine_id, committee2_ops);
        CommitteeOps::<TestRuntime>::insert(&*committee3, &machine_id, committee3_ops);

        let summary = Summary {
            valid_vote: vec![*committee2, *committee1],
            invalid_vote: vec![*committee3],
            info: Some(committee_ops.machine_info),
            verify_result: VerifyResult::Confirmed,
            ..Default::default()
        };

        assert_eq!(OnlineCommittee::summary_confirmation2(&machine_id), summary);
    })
}

// case 4: 3个支持，内容都不一致 -> 无共识 + 重新分配
#[test]
fn test_summary_confirmation4() {
    new_test_with_init_params_ext().execute_with(|| {
        let machine_id = get_machine_id();

        run_to_block(10);

        // 构建 machine_committee
        <MachineCommittee<TestRuntime>>::insert(
            &machine_id,
            OCMachineCommitteeList {
                book_time: 9,
                booked_committee: vec![*committee3, *committee2, *committee1],
                hashed_committee: vec![*committee3, *committee2, *committee1],
                confirm_start_time: 5432,
                confirmed_committee: vec![*committee3, *committee2, *committee1],
                onlined_committee: vec![],
                status: OCVerifyStatus::Summarizing,
            },
        );

        let machine_info_hash: [u8; 16] =
            hex::decode("d80b116fd318f19fd89da792aba5e875").unwrap().try_into().unwrap();
        let committee_upload_info = get_base_machine_info();

        let committee_ops = OCCommitteeOps {
            staked_dbc: 1000 * ONE_DBC,
            verify_time: vec![],
            confirm_hash: machine_info_hash,
            hash_time: 16887,
            confirm_time: 16891,
            machine_status: OCMachineStatus::Confirmed,
            machine_info: committee_upload_info,
        };

        let committee1_ops = OCCommitteeOps {
            verify_time: vec![1622, 3062, 4502],
            confirm_hash: machine_info_hash,
            ..committee_ops.clone()
        };

        let committee2_ops = OCCommitteeOps {
            verify_time: vec![1142, 2582, 4022],
            confirm_hash: machine_info_hash,

            machine_info: CommitteeUploadInfo { gpu_num: 5, ..committee_ops.machine_info.clone() },
            ..committee_ops.clone()
        };

        let committee3_ops = OCCommitteeOps {
            verify_time: vec![662, 2102, 3542],
            confirm_hash: machine_info_hash,
            machine_info: CommitteeUploadInfo { gpu_num: 3, ..committee_ops.machine_info.clone() },
            ..committee_ops
        };

        // 构建committee_ops
        CommitteeOps::<TestRuntime>::insert(&*committee1, &machine_id, committee1_ops);
        CommitteeOps::<TestRuntime>::insert(&*committee2, &machine_id, committee2_ops);
        CommitteeOps::<TestRuntime>::insert(&*committee3, &machine_id, committee3_ops);

        assert_eq!(
            OnlineCommittee::summary_confirmation2(&machine_id),
            Summary {
                invalid_vote: vec![*committee3, *committee2, *committee1],
                verify_result: VerifyResult::NoConsensus,
                ..Default::default()
            },
        );
    })
}

// case 5: 2个支持，1个反对（2个不一致） -> 无共识 + 重新分配
#[test]
fn test_summary_confirmation5() {
    new_test_with_init_params_ext().execute_with(|| {
        run_to_block(10);

        let machine_id = get_machine_id();

        // 构建 machine_committee
        <MachineCommittee<TestRuntime>>::insert(
            &machine_id,
            OCMachineCommitteeList {
                book_time: 9,
                booked_committee: vec![*committee3, *committee2, *committee1],
                hashed_committee: vec![*committee3, *committee2, *committee1],
                confirm_start_time: 5432,
                confirmed_committee: vec![*committee3, *committee2, *committee1],
                onlined_committee: vec![],
                status: OCVerifyStatus::Summarizing,
            },
        );

        let machine_info_hash: [u8; 16] =
            hex::decode("d80b116fd318f19fd89da792aba5e875").unwrap().try_into().unwrap();
        let committee_upload_info = get_base_machine_info();

        let committee_ops = OCCommitteeOps {
            staked_dbc: 1000 * ONE_DBC,
            verify_time: vec![],
            confirm_hash: machine_info_hash,
            hash_time: 16887,
            confirm_time: 16891,
            machine_status: OCMachineStatus::Confirmed,
            machine_info: committee_upload_info,
        };

        let committee1_ops = OCCommitteeOps {
            verify_time: vec![1622, 3062, 4502],
            confirm_hash: machine_info_hash,
            machine_info: CommitteeUploadInfo { gpu_num: 4, ..committee_ops.machine_info.clone() },

            ..committee_ops.clone()
        };

        let committee2_ops = OCCommitteeOps {
            verify_time: vec![1142, 2582, 4022],
            confirm_hash: machine_info_hash,
            machine_info: CommitteeUploadInfo { gpu_num: 3, ..committee_ops.machine_info.clone() },

            ..committee_ops.clone()
        };

        let committee3_ops = OCCommitteeOps {
            verify_time: vec![662, 2102, 3542],
            confirm_hash: machine_info_hash,
            machine_info: CommitteeUploadInfo {
                gpu_num: 3,
                is_support: false,
                ..committee_ops.machine_info.clone()
            },
            ..committee_ops
        };

        // 构建committee_ops
        CommitteeOps::<TestRuntime>::insert(&*committee1, &*machine_id, committee1_ops);
        CommitteeOps::<TestRuntime>::insert(&*committee2, &*machine_id, committee2_ops);
        CommitteeOps::<TestRuntime>::insert(&*committee3, &*machine_id, committee3_ops);

        let summary = Summary {
            invalid_vote: vec![*committee3, *committee2, *committee1],
            verify_result: VerifyResult::NoConsensus,
            ..Default::default()
        };

        assert_eq!(OnlineCommittee::summary_confirmation2(&machine_id), summary);
    })
}

// case 6: 2个反对，1个支持 -> 不上线 + 奖励 + 惩罚
#[test]
fn test_summary_confirmation6() {
    new_test_with_init_params_ext().execute_with(|| {
        run_to_block(10);
        let machine_id = get_machine_id();

        // 构建 machine_committee
        <MachineCommittee<TestRuntime>>::insert(
            &machine_id,
            OCMachineCommitteeList {
                book_time: 9,
                booked_committee: vec![*committee3, *committee2, *committee1],
                hashed_committee: vec![*committee3, *committee2, *committee1],
                confirm_start_time: 5432,
                confirmed_committee: vec![*committee3, *committee2, *committee1],
                onlined_committee: vec![],
                status: OCVerifyStatus::Summarizing,
            },
        );

        let machine_info_hash: [u8; 16] =
            hex::decode("d80b116fd318f19fd89da792aba5e875").unwrap().try_into().unwrap();
        let committee_upload_info = get_base_machine_info();

        let committee_ops = OCCommitteeOps {
            staked_dbc: 1000 * ONE_DBC,
            verify_time: vec![],
            confirm_hash: machine_info_hash,
            hash_time: 16887,
            confirm_time: 16891,
            machine_status: OCMachineStatus::Confirmed,
            machine_info: committee_upload_info,
        };

        let committee1_ops = OCCommitteeOps {
            verify_time: vec![1622, 3062, 4502],
            confirm_hash: machine_info_hash,
            machine_info: CommitteeUploadInfo {
                gpu_num: 3,
                is_support: false,
                ..committee_ops.machine_info.clone()
            },
            ..committee_ops.clone()
        };

        let committee2_ops = OCCommitteeOps {
            verify_time: vec![1142, 2582, 4022],
            confirm_hash: machine_info_hash,
            machine_info: CommitteeUploadInfo { gpu_num: 3, ..committee_ops.machine_info.clone() },
            ..committee_ops.clone()
        };

        let committee3_ops = OCCommitteeOps {
            verify_time: vec![662, 2102, 3542],
            confirm_hash: machine_info_hash,
            machine_info: CommitteeUploadInfo {
                gpu_num: 3,
                is_support: false,
                ..committee_ops.machine_info.clone()
            },
            ..committee_ops
        };

        // 构建committee_ops
        CommitteeOps::<TestRuntime>::insert(&*committee1, &*machine_id, committee1_ops);
        CommitteeOps::<TestRuntime>::insert(&*committee2, &*machine_id, committee2_ops);
        CommitteeOps::<TestRuntime>::insert(&*committee3, &*machine_id, committee3_ops);

        let summary = Summary {
            valid_vote: vec![*committee3, *committee1],
            invalid_vote: vec![*committee2],
            verify_result: VerifyResult::Refused,
            ..Default::default()
        };

        assert_eq!(OnlineCommittee::summary_confirmation2(&machine_id), summary.clone());
    })
}

// case 7: 3个反对 -> 不上线 + 奖励
#[test]
fn test_summary_confirmation7() {
    new_test_with_init_params_ext().execute_with(|| {
        let machine_id = get_machine_id();

        run_to_block(10);

        // 构建 machine_committee
        <MachineCommittee<TestRuntime>>::insert(
            &machine_id,
            OCMachineCommitteeList {
                book_time: 9,
                booked_committee: vec![*committee3, *committee2, *committee1],
                hashed_committee: vec![*committee3, *committee2, *committee1],
                confirm_start_time: 5432,
                confirmed_committee: vec![*committee3, *committee2, *committee1],
                onlined_committee: vec![],
                status: OCVerifyStatus::Summarizing,
            },
        );

        let machine_info_hash: [u8; 16] =
            hex::decode("d80b116fd318f19fd89da792aba5e875").unwrap().try_into().unwrap();
        let committee_upload_info = get_base_machine_info();

        let committee_ops = OCCommitteeOps {
            staked_dbc: 1000 * ONE_DBC,
            verify_time: vec![],
            confirm_hash: machine_info_hash,
            hash_time: 16887,
            confirm_time: 16891,
            machine_status: OCMachineStatus::Confirmed,
            machine_info: committee_upload_info,
        };

        let committee1_ops = OCCommitteeOps {
            verify_time: vec![1622, 3062, 4502],
            confirm_hash: machine_info_hash,
            machine_info: CommitteeUploadInfo {
                gpu_num: 3,
                is_support: false,
                ..committee_ops.machine_info.clone()
            },
            ..committee_ops.clone()
        };

        let committee2_ops = OCCommitteeOps {
            verify_time: vec![1142, 2582, 4022],
            confirm_hash: machine_info_hash,
            machine_info: CommitteeUploadInfo {
                gpu_num: 3,
                is_support: false,
                ..committee_ops.machine_info.clone()
            },
            ..committee_ops.clone()
        };

        let committee3_ops = OCCommitteeOps {
            verify_time: vec![662, 2102, 3542],
            confirm_hash: machine_info_hash,
            machine_info: CommitteeUploadInfo {
                gpu_num: 3,
                is_support: false,
                ..committee_ops.machine_info.clone()
            },
            ..committee_ops
        };

        // 构建committee_ops
        CommitteeOps::<TestRuntime>::insert(&*committee1, &machine_id, committee1_ops);
        CommitteeOps::<TestRuntime>::insert(&*committee2, &machine_id, committee2_ops);
        CommitteeOps::<TestRuntime>::insert(&*committee3, &machine_id, committee3_ops);

        let summary = Summary {
            valid_vote: vec![*committee3, *committee2, *committee1],
            verify_result: VerifyResult::Refused,
            ..Default::default()
        };

        assert_eq!(OnlineCommittee::summary_confirmation2(&machine_id), summary);
    })
}

// case 8: 2提交Hash， 2提交原始值，且都是反对
#[test]
fn test_summary_confirmation8() {
    new_test_with_init_params_ext().execute_with(|| {
        let machine_id = get_machine_id();

        run_to_block(10);

        // 构建 machine_committee
        <MachineCommittee<TestRuntime>>::insert(
            &machine_id,
            OCMachineCommitteeList {
                book_time: 9,
                booked_committee: vec![*committee3, *committee2, *committee1],
                hashed_committee: vec![*committee3, *committee2],
                confirm_start_time: 5432,
                confirmed_committee: vec![*committee3, *committee2],
                onlined_committee: vec![],
                status: OCVerifyStatus::Summarizing,
            },
        );

        let machine_info_hash: [u8; 16] =
            hex::decode("d80b116fd318f19fd89da792aba5e875").unwrap().try_into().unwrap();
        let committee_upload_info = get_base_machine_info();

        let committee_ops = OCCommitteeOps {
            staked_dbc: 1000 * ONE_DBC,
            verify_time: vec![],
            confirm_hash: machine_info_hash,
            hash_time: 16887,
            confirm_time: 16891,
            machine_status: OCMachineStatus::Confirmed,
            machine_info: committee_upload_info,
        };

        let committee2_ops = OCCommitteeOps {
            verify_time: vec![1142, 2582, 4022],
            confirm_hash: machine_info_hash,
            machine_info: CommitteeUploadInfo {
                gpu_num: 3,
                is_support: false,
                ..committee_ops.machine_info.clone()
            },
            ..committee_ops.clone()
        };

        let committee3_ops = OCCommitteeOps {
            verify_time: vec![662, 2102, 3542],
            confirm_hash: machine_info_hash,
            machine_info: CommitteeUploadInfo {
                gpu_num: 3,
                is_support: false,
                ..committee_ops.machine_info.clone()
            },
            ..committee_ops
        };

        // 构建committee_ops
        CommitteeOps::<TestRuntime>::insert(&*committee2, &machine_id, committee2_ops);
        CommitteeOps::<TestRuntime>::insert(&*committee3, &machine_id, committee3_ops);

        let summary = Summary {
            unruly: vec![*committee1],
            valid_vote: vec![*committee3, *committee2],
            verify_result: VerifyResult::Refused,

            ..Default::default()
        };

        assert_eq!(OnlineCommittee::summary_confirmation2(&machine_id), summary);
    })
}

// case 9: 2提交Hash，2提交原始值，且都是支持
#[test]
fn test_summary_confirmation9() {
    new_test_with_init_params_ext().execute_with(|| {
        let machine_id = get_machine_id();

        run_to_block(10);

        // 构建 machine_committee
        <MachineCommittee<TestRuntime>>::insert(
            &machine_id,
            OCMachineCommitteeList {
                book_time: 9,
                booked_committee: vec![*committee3, *committee2, *committee1],
                hashed_committee: vec![*committee3, *committee2],
                confirm_start_time: 5432,
                confirmed_committee: vec![*committee3, *committee2],
                onlined_committee: vec![],
                status: OCVerifyStatus::Summarizing,
            },
        );

        let machine_info_hash: [u8; 16] =
            hex::decode("d80b116fd318f19fd89da792aba5e875").unwrap().try_into().unwrap();
        let committee_upload_info = get_base_machine_info();

        let committee_ops = OCCommitteeOps {
            staked_dbc: 1000 * ONE_DBC,
            verify_time: vec![],
            confirm_hash: machine_info_hash,
            hash_time: 16887,
            confirm_time: 16891,
            machine_status: OCMachineStatus::Confirmed,
            machine_info: committee_upload_info,
        };

        let committee2_ops = OCCommitteeOps {
            verify_time: vec![1142, 2582, 4022],
            confirm_hash: machine_info_hash,
            ..committee_ops.clone()
        };

        let committee3_ops = OCCommitteeOps {
            verify_time: vec![662, 2102, 3542],
            confirm_hash: machine_info_hash,
            ..committee_ops.clone()
        };

        // 构建committee_ops
        <CommitteeOps<TestRuntime>>::insert(&*committee2, &*machine_id, committee2_ops);
        <CommitteeOps<TestRuntime>>::insert(&*committee3, &*machine_id, committee3_ops);

        let summary = Summary {
            valid_vote: vec![*committee3, *committee2],
            unruly: vec![*committee1],
            info: Some(committee_ops.machine_info),
            verify_result: VerifyResult::Confirmed,
            ..Default::default()
        };

        assert_eq!(OnlineCommittee::summary_confirmation2(&machine_id), summary);
    })
}

// case 10: 3提交Hash，2提交原始值，且都是支持，且两个互不相等
#[test]
fn test_summary_confirmation10() {
    new_test_with_init_params_ext().execute_with(|| {
        let machine_id = get_machine_id();

        run_to_block(10);

        // 构建 machine_committee
        <MachineCommittee<TestRuntime>>::insert(
            &machine_id,
            OCMachineCommitteeList {
                book_time: 9,
                booked_committee: vec![*committee3, *committee2, *committee1],
                hashed_committee: vec![*committee3, *committee2],
                confirm_start_time: 5432,
                confirmed_committee: vec![*committee3, *committee2],
                onlined_committee: vec![],
                status: OCVerifyStatus::Summarizing,
            },
        );

        let machine_info_hash: [u8; 16] =
            hex::decode("d80b116fd318f19fd89da792aba5e875").unwrap().try_into().unwrap();
        let committee_upload_info = get_base_machine_info();

        let committee_ops = OCCommitteeOps {
            staked_dbc: 1000 * ONE_DBC,
            verify_time: vec![],
            confirm_hash: machine_info_hash,
            hash_time: 16887,
            confirm_time: 16891,
            machine_status: OCMachineStatus::Confirmed,
            machine_info: committee_upload_info,
        };

        let committee2_ops = OCCommitteeOps {
            verify_time: vec![1142, 2582, 4022],
            confirm_hash: machine_info_hash,
            machine_info: CommitteeUploadInfo { gpu_num: 3, ..committee_ops.machine_info.clone() },
            ..committee_ops.clone()
        };

        let committee3_ops = OCCommitteeOps {
            verify_time: vec![662, 2102, 3542],
            confirm_hash: machine_info_hash,
            ..committee_ops
        };

        // 构建committee_ops
        <CommitteeOps<TestRuntime>>::insert(&*committee2, &machine_id, committee2_ops);
        <CommitteeOps<TestRuntime>>::insert(&*committee3, &machine_id, committee3_ops);

        assert_eq!(
            OnlineCommittee::summary_confirmation2(&machine_id),
            Summary {
                unruly: vec![*committee1],
                invalid_vote: vec![*committee3, *committee2],
                verify_result: VerifyResult::NoConsensus,
                ..Default::default()
            }
        );
    })
}

// case 11: 3提交Hash，2提交原始值，且都是支持，且两个相等
#[test]
fn test_summary_confirmation11() {
    new_test_with_init_params_ext().execute_with(|| {
        let machine_id = get_machine_id();

        run_to_block(10);

        // 构建 machine_committee
        <MachineCommittee<TestRuntime>>::insert(
            &machine_id,
            OCMachineCommitteeList {
                book_time: 9,
                booked_committee: vec![*committee3, *committee2, *committee1],
                hashed_committee: vec![*committee3, *committee2],
                confirm_start_time: 5432,
                confirmed_committee: vec![*committee3, *committee2],
                onlined_committee: vec![],
                status: OCVerifyStatus::Summarizing,
            },
        );

        let machine_info_hash: [u8; 16] =
            hex::decode("d80b116fd318f19fd89da792aba5e875").unwrap().try_into().unwrap();
        let committee_upload_info = get_base_machine_info();

        let committee_ops = OCCommitteeOps {
            staked_dbc: 1000 * ONE_DBC,
            verify_time: vec![],
            confirm_hash: machine_info_hash,
            hash_time: 16887,
            confirm_time: 16891,
            machine_status: OCMachineStatus::Confirmed,
            machine_info: committee_upload_info,
        };

        let committee2_ops = OCCommitteeOps {
            verify_time: vec![1142, 2582, 4022],
            confirm_hash: machine_info_hash,
            ..committee_ops.clone()
        };

        let committee3_ops = OCCommitteeOps {
            verify_time: vec![662, 2102, 3542],
            confirm_hash: machine_info_hash,
            ..committee_ops.clone()
        };

        // 构建committee_ops
        <CommitteeOps<TestRuntime>>::insert(&*committee2, &machine_id, committee2_ops);
        <CommitteeOps<TestRuntime>>::insert(&*committee3, &machine_id, committee3_ops);

        assert_eq!(
            OnlineCommittee::summary_confirmation2(&machine_id),
            Summary {
                unruly: vec![*committee1],
                valid_vote: vec![*committee3, *committee2],
                info: Some(committee_ops.machine_info),
                verify_result: VerifyResult::Confirmed,
                ..Default::default()
            }
        );
    })
}

// 机器成功上线，拒绝的委员会惩罚被执行
#[test]
fn test_machine_online_succeed_slash_execed() {
    new_test_with_online_machine_distribution().execute_with(|| {
        let committee1_box_pubkey =
            hex::decode("ff3033c763f71bc51f372c1dc5095accc26880e138df84cac13c46bfd7dbd74f")
                .unwrap()
                .try_into()
                .unwrap();
        let committee2_box_pubkey =
            hex::decode("336404f7d316565cc3c3350e70561f4177803e0bb02a7f2e4e02a4f0e361157e")
                .unwrap()
                .try_into()
                .unwrap();
        let committee3_box_pubkey =
            hex::decode("a7804e30caa5645e97489b2d4711e3d8f4e17a683338cba97a53b960648f0438")
                .unwrap()
                .try_into()
                .unwrap();
        let committee4_box_pubkey =
            hex::decode("5eec53877f4b18c8b003fa983d27ef2e5518b7e4d08d482922a7787f2ea75529")
                .unwrap()
                .try_into()
                .unwrap();

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
            Origin::signed(*committee1),
            machine_id.clone(),
            machine_info_hash1
        ));
        let machine_info_hash2: [u8; 16] =
            hex::decode("c016090e0943c17f5d4999dc6eb52683").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(*committee2),
            machine_id.clone(),
            machine_info_hash2
        ));

        let machine_info_hash3: [u8; 16] =
            hex::decode("fe3d8c7eb5dc36f3f923aff6f3367544").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(*committee4),
            machine_id.clone(),
            machine_info_hash3
        ));

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

        // 第三个委员会提交错误的机器信息
        committee_upload_info.rand_str = "abcdefg3".as_bytes().to_vec();
        committee_upload_info.mem_num = 450;
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            Origin::signed(*committee4),
            committee_upload_info.clone()
        ));

        run_to_block(11);
        let current_machine_info = OnlineProfile::machines_info(&machine_id);
        assert_eq!(
            OnlineProfile::live_machines(),
            LiveMachine { online_machine: vec![machine_id.clone()], ..Default::default() }
        );
        assert_eq!(current_machine_info.reward_committee, vec![*committee2, *committee1]);

        assert_eq!(
            OnlineCommittee::pending_slash(0),
            crate::OCPendingSlashInfo {
                machine_id,
                stash_slash_amount: 0,

                inconsistent_committee: vec![*committee4],
                reward_committee: vec![*committee2, *committee1],
                committee_stake: 1000 * ONE_DBC,

                slash_time: 11,
                slash_exec_time: 11 + 2880 * 2,

                book_result: crate::OCBookResultType::OnlineSucceed,
                slash_result: crate::OCSlashResult::Pending,

                ..Default::default()
            }
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
                used_stake: 0,
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
                can_claim_reward: 1375 * ONE_DBC,
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
            hex::decode("ff3033c763f71bc51f372c1dc5095accc26880e138df84cac13c46bfd7dbd74f")
                .unwrap()
                .try_into()
                .unwrap();
        let committee2_box_pubkey =
            hex::decode("336404f7d316565cc3c3350e70561f4177803e0bb02a7f2e4e02a4f0e361157e")
                .unwrap()
                .try_into()
                .unwrap();
        let committee3_box_pubkey =
            hex::decode("5eec53877f4b18c8b003fa983d27ef2e5518b7e4d08d482922a7787f2ea75529")
                .unwrap()
                .try_into()
                .unwrap();

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
            is_support: false,
        };

        // 委员会提交机器Hash
        // 委员会1，2反对，3支持
        let machine_info_hash1: [u8; 16] =
            hex::decode("cee14a520ba6a988c306aab9dc3794b1").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(*committee1),
            machine_id.clone(),
            machine_info_hash1
        ));
        let machine_info_hash2: [u8; 16] =
            hex::decode("8c7e7ca563169689f1c789f8d4f510f8").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(*committee2),
            machine_id.clone(),
            machine_info_hash2
        ));

        let machine_info_hash3: [u8; 16] =
            hex::decode("4a6b2df1e1a77b9bcdab5e31dc7950d2").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(*committee4),
            machine_id.clone(),
            machine_info_hash3
        ));

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

        // 第三个委员会提交错误的机器信息
        committee_upload_info.rand_str = "abcdefg3".as_bytes().to_vec();
        committee_upload_info.is_support = true;
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            Origin::signed(*committee4),
            committee_upload_info
        ));

        run_to_block(11);
        // 机器被拒绝上线，将会产生对委员会3和satsh账户的pending_slash
        assert_eq!(
            OnlineCommittee::pending_slash(0),
            crate::OCPendingSlashInfo {
                machine_id: machine_id.clone(),
                machine_stash: *stash,
                stash_slash_amount: 5000 * ONE_DBC, // 10,0000 * 5 / 100

                inconsistent_committee: vec![*committee4],
                unruly_committee: vec![],
                reward_committee: vec![*committee2, *committee1],
                committee_stake: 1000 * ONE_DBC,

                slash_time: 11,
                slash_exec_time: 11 + 2880 * 2,

                book_result: crate::OCBookResultType::OnlineRefused,
                slash_result: crate::OCSlashResult::Pending,
            }
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
            Committee::committee_stake(&*committee2),
            committee::CommitteeStakeInfo {
                box_pubkey: committee2_box_pubkey,
                staked_amount: 20000 * ONE_DBC,
                used_stake: 1000 * ONE_DBC,
                can_claim_reward: 0,
                claimed_reward: 0,
            }
        );
        assert_eq!(
            Committee::committee_stake(&*committee4),
            committee::CommitteeStakeInfo {
                box_pubkey: committee3_box_pubkey,
                staked_amount: 20000 * ONE_DBC,
                used_stake: 1000 * ONE_DBC,
                can_claim_reward: 0,
                claimed_reward: 0,
            }
        );

        assert_eq!(Balances::reserved_balance(&*committee1), 20000 * ONE_DBC);
        assert_eq!(Balances::reserved_balance(&*committee2), 20000 * ONE_DBC);
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
            Committee::committee_stake(&*committee4),
            committee::CommitteeStakeInfo {
                box_pubkey: committee3_box_pubkey,
                staked_amount: 19000 * ONE_DBC,
                used_stake: 0,
                can_claim_reward: 0,
                claimed_reward: 0,
            }
        );

        assert_eq!(Balances::reserved_balance(&*committee1), 20000 * ONE_DBC);
        assert_eq!(Balances::reserved_balance(&*committee2), 20000 * ONE_DBC);
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
            Origin::signed(*committee1),
            machine_id.clone(),
            machine_info_hash1
        ));
        let machine_info_hash2: [u8; 16] =
            hex::decode("c016090e0943c17f5d4999dc6eb52683").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(*committee2),
            machine_id.clone(),
            machine_info_hash2
        ));

        let machine_info_hash3: [u8; 16] =
            hex::decode("fe3d8c7eb5dc36f3f923aff6f3367544").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(*committee4),
            machine_id,
            machine_info_hash3
        ));

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

        // 第三个委员会提交错误的机器信息
        committee_upload_info.rand_str = "abcdefg3".as_bytes().to_vec();
        committee_upload_info.mem_num = 450;
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            Origin::signed(*committee4),
            committee_upload_info
        ));

        run_to_block(12);

        // committee 3 apply_slash_review
        let slash_reason = "They are wrong.".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::apply_slash_review(
            Origin::signed(*committee4),
            0,
            slash_reason.clone()
        ));

        assert_eq!(
            OnlineCommittee::pending_slash_review(0),
            crate::OCPendingSlashReviewInfo {
                applicant: *committee4,
                staked_amount: 1000 * ONE_DBC,
                apply_time: 13,
                expire_time: 11 + 2880 * 2,
                reason: slash_reason,
            }
        );
        // 检查commttee reserve
        assert_eq!(Balances::reserved_balance(&*committee4), (20000 + 1000) * ONE_DBC);

        assert_ok!(OnlineCommittee::do_cancel_slash(0));
    })
}
