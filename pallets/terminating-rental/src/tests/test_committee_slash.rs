use crate::{
    IRBookResultType, IRCommitteeMachineList, IRCommitteeUploadInfo, IRLiveMachine,
    IRMachineCommitteeList, IRPendingSlashInfo, IRSlashResult, IRStakerCustomizeInfo,
    IRVerifyStatus,
};

use super::super::mock::{TerminatingRental as IRMachine, INIT_BALANCE, *};
use frame_support::assert_ok;
use std::convert::TryInto;

pub fn new_test_after_machine_distribute() -> sp_io::TestExternalities {
    let mut ext = new_test_with_init_params_ext();

    ext.execute_with(|| {
        let stash = sr25519::Public::from(Sr25519Keyring::Ferdie);
        let controller = sr25519::Public::from(Sr25519Keyring::Eve);

        assert_ok!(IRMachine::set_controller(Origin::signed(stash), controller));
        assert_ok!(IRMachine::gen_server_room(Origin::signed(controller)));
        let server_rooms = IRMachine::stash_server_rooms(stash);

        // Bob pubkey
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();
        let msg = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48\
                   5CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL";
        let sig = "b4084f70730b183127e9db78c6d8dcf79039f23466cd1ee8b536c40c3027a83d\
                   ab040be4ed2db57b67eaac406817a69ce72a13f8ac11ba460e15d318b1504481";

        let _committee1 = sr25519::Public::from(Sr25519Keyring::Alice);
        let committee2 = sr25519::Public::from(Sr25519Keyring::Charlie);
        let committee3 = sr25519::Public::from(Sr25519Keyring::Dave);
        let committee4 = sr25519::Public::from(Sr25519Keyring::Eve);

        assert_ok!(IRMachine::bond_machine(
            Origin::signed(controller),
            machine_id.clone(),
            msg.as_bytes().to_vec(),
            hex::decode(sig).unwrap()
        ));

        assert_ok!(IRMachine::add_machine_info(
            Origin::signed(controller),
            machine_id.clone(),
            IRStakerCustomizeInfo {
                server_room: server_rooms[0],
                upload_net: 100,
                download_net: 100,
                longitude: crate::IRLongitude::East(1157894),
                latitude: crate::IRLatitude::North(235678),
                telecom_operators: vec!["China Unicom".into()],
            }
        ));

        // 自动派单
        run_to_block(3);
        {
            assert_eq!(
                IRMachine::machine_committee(&machine_id),
                IRMachineCommitteeList {
                    book_time: 2,
                    booked_committee: vec![committee3, committee2, committee4],
                    confirm_start_time: 4320 + 2, // 360 * 12
                    ..Default::default()
                }
            );
        }
    });
    ext
}

//  1. Commttee not submit hash works
#[test]
fn committee_not_submit_slash_works() {
    new_test_after_machine_distribute().execute_with(|| {
        let _stash = sr25519::Public::from(Sr25519Keyring::Ferdie);
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();

        let committee2 = sr25519::Public::from(Sr25519Keyring::Charlie);
        let committee3 = sr25519::Public::from(Sr25519Keyring::Dave);
        let committee4 = sr25519::Public::from(Sr25519Keyring::Eve);

        // committee 1, 2提交Hash
        // 委员会添加机器Hash
        let hash1: [u8; 16] =
            hex::decode("53cf058dfa07ef517b2f28bccff88c2b").unwrap().try_into().unwrap();
        let hash2: [u8; 16] =
            hex::decode("3f775d3f4a144b94d6d551f6091a5126").unwrap().try_into().unwrap();

        assert_ok!(IRMachine::submit_confirm_hash(
            Origin::signed(committee2),
            machine_id.clone(),
            hash1
        ));

        assert_ok!(IRMachine::submit_confirm_hash(
            Origin::signed(committee3),
            machine_id.clone(),
            hash2
        ));

        run_to_block(3 + 4320);

        // 现在机器状态将变成SubmittingRaw以允许提交原始值
        {
            assert_eq!(
                IRMachine::machine_committee(&machine_id),
                IRMachineCommitteeList {
                    book_time: 2,
                    booked_committee: vec![committee3, committee2, committee4],
                    hashed_committee: vec![committee3, committee2],
                    confirm_start_time: 4320 + 2, // 360 * 12
                    status: IRVerifyStatus::SubmittingRaw,
                    ..Default::default()
                }
            );
        }

        // 两个委员会提交原始值
        let mut upload_info = IRCommitteeUploadInfo {
            machine_id: machine_id.clone(),
            gpu_type: "GeForceRTX3080".as_bytes().to_vec(),
            gpu_num: 8,
            cuda_core: 8704,
            gpu_mem: 10,
            calc_point: 119780,
            sys_disk: 500,
            data_disk: 3905,
            cpu_type: "Intel(R) Xeon(R) Silver 4214R".as_bytes().to_vec(),
            cpu_core_num: 46,
            cpu_rate: 2400,
            mem_num: 440,

            rand_str: "abcdefg1".as_bytes().to_vec(),
            is_support: true,
        };

        assert_ok!(IRMachine::submit_confirm_raw(Origin::signed(committee2), upload_info.clone()));
        upload_info.rand_str = "abcdefg2".as_bytes().to_vec();
        assert_ok!(IRMachine::submit_confirm_raw(Origin::signed(committee3), upload_info.clone()));

        // 现在应该添加了惩罚，并且状态变为可提交原始值
        run_to_block(4 + 4320);
        {
            // 机器成功上线
            assert_eq!(
                IRMachine::live_machines(),
                IRLiveMachine { online_machine: vec![machine_id.clone()], ..Default::default() }
            );
            assert_eq!(
                IRMachine::pending_slash(0),
                IRPendingSlashInfo {
                    machine_id: machine_id.clone(),
                    inconsistent_committee: vec![],
                    unruly_committee: vec![committee4],
                    reward_committee: vec![committee3, committee2],
                    committee_stake: 1000 * ONE_DBC,

                    slash_time: 4 + 4320,
                    slash_exec_time: 4 + 4320 + 2880 * 2,

                    book_result: IRBookResultType::OnlineSucceed,
                    slash_result: IRSlashResult::Pending,
                    ..Default::default()
                }
            );
            assert_eq!(IRMachine::unhandled_slash(), vec![0]);
            assert_eq!(
                IRMachine::committee_machine(&committee4),
                IRCommitteeMachineList::default()
            );
        }

        // 自动执行惩罚: committee4 被惩罚，惩罚到国库
        run_to_block(4 + 4320 + 2880 * 2 + 1);
        {
            // committee4 is also machien controller
            assert_eq!(
                Balances::free_balance(committee4),
                INIT_BALANCE - 20000 * ONE_DBC - 10 * ONE_DBC
            );
            assert_eq!(Balances::free_balance(committee2), INIT_BALANCE - 20000 * ONE_DBC);
            assert_eq!(Balances::free_balance(committee3), INIT_BALANCE - 20000 * ONE_DBC);

            assert_eq!(Balances::reserved_balance(committee4), 20000 * ONE_DBC - 1000 * ONE_DBC);
            assert_eq!(Balances::reserved_balance(committee2), 20000 * ONE_DBC);
            assert_eq!(Balances::reserved_balance(committee3), 20000 * ONE_DBC);
        }
    })
}

//  2. Commttee not submit hash
#[test]
fn committee_not_submit_raw_slash_works() {
    new_test_after_machine_distribute().execute_with(|| {
        //
    })
}

pub fn new_test_after_machien_online() -> sp_io::TestExternalities {
    let mut ext = new_test_with_init_params_ext();

    ext.execute_with(|| {
        let stash = sr25519::Public::from(Sr25519Keyring::Ferdie);
        let controller = sr25519::Public::from(Sr25519Keyring::Eve);

        // Bob pubkey
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();
        let msg = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48\
                   5CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL";
        let sig = "b4084f70730b183127e9db78c6d8dcf79039f23466cd1ee8b536c40c3027a83d\
                   ab040be4ed2db57b67eaac406817a69ce72a13f8ac11ba460e15d318b1504481";

        let committee1 = sr25519::Public::from(Sr25519Keyring::Alice);
        let committee2 = sr25519::Public::from(Sr25519Keyring::Charlie);
        // let committee3 = sr25519::Public::from(Sr25519Keyring::Dave);
        let committee4 = sr25519::Public::from(Sr25519Keyring::Eve);

        assert_ok!(IRMachine::bond_machine(
            Origin::signed(controller),
            machine_id.clone(),
            msg.as_bytes().to_vec(),
            hex::decode(sig).unwrap()
        ));

        assert_ok!(IRMachine::gen_server_room(Origin::signed(controller)));
        let server_rooms = IRMachine::stash_server_rooms(stash);

        // 添加机器信息
        assert_ok!(IRMachine::add_machine_info(
            Origin::signed(controller),
            machine_id.clone(),
            IRStakerCustomizeInfo {
                server_room: server_rooms[0],
                upload_net: 100,
                download_net: 100,
                longitude: crate::IRLongitude::East(1157894),
                latitude: crate::IRLatitude::North(235678),
                telecom_operators: vec!["China Unicom".into()],
            }
        ));

        // 自动派单
        run_to_block(3);

        // 委员会添加机器Hash
        let hash1: [u8; 16] =
            hex::decode("53cf058dfa07ef517b2f28bccff88c2b").unwrap().try_into().unwrap();
        let hash2: [u8; 16] =
            hex::decode("3f775d3f4a144b94d6d551f6091a5126").unwrap().try_into().unwrap();
        let hash3: [u8; 16] =
            hex::decode("4983040157403addac94ca860ddbff7f").unwrap().try_into().unwrap();

        assert_ok!(IRMachine::submit_confirm_hash(
            Origin::signed(committee1),
            machine_id.clone(),
            hash1
        ));

        assert_ok!(IRMachine::submit_confirm_hash(
            Origin::signed(committee2),
            machine_id.clone(),
            hash2
        ));

        assert_ok!(IRMachine::submit_confirm_hash(
            Origin::signed(committee4),
            machine_id.clone(),
            hash3
        ));

        // 委员会提交原始信息
        let mut upload_info = IRCommitteeUploadInfo {
            machine_id: machine_id.clone(),
            gpu_type: "GeForceRTX3080".as_bytes().to_vec(),
            gpu_num: 8,
            cuda_core: 8704,
            gpu_mem: 10,
            calc_point: 119780,
            sys_disk: 500,
            data_disk: 3905,
            cpu_type: "Intel(R) Xeon(R) Silver 4214R".as_bytes().to_vec(),
            cpu_core_num: 46,
            cpu_rate: 2400,
            mem_num: 440,

            rand_str: "abcdefg1".as_bytes().to_vec(),
            is_support: true,
        };

        // 委员会添加机器原始值
        assert_ok!(IRMachine::submit_confirm_raw(Origin::signed(committee1), upload_info.clone()));

        upload_info.rand_str = "abcdefg2".as_bytes().to_vec();
        assert_ok!(IRMachine::submit_confirm_raw(Origin::signed(committee2), upload_info.clone()));
        upload_info.rand_str = "abcdefg3".as_bytes().to_vec();
        assert_ok!(IRMachine::submit_confirm_raw(Origin::signed(committee4), upload_info));

        run_to_block(4);
    });

    ext
}
