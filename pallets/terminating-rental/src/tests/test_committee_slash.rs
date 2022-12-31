use crate::{IRLiveMachine, OCCommitteeMachineList};
use dbc_support::{
    machine_type::{CommitteeUploadInfo, Latitude, Longitude, MachineStatus, StakerCustomizeInfo},
    verify_committee_slash::{OCPendingSlashInfo, OCSlashResult},
    verify_online::{OCBookResultType, OCMachineCommitteeList, OCVerifyStatus, StashMachine},
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
            StakerCustomizeInfo {
                server_room: server_rooms[0],
                upload_net: 100,
                download_net: 100,
                longitude: Longitude::East(1157894),
                latitude: Latitude::North(235678),
                telecom_operators: vec!["China Unicom".into()],
            }
        ));

        // 自动派单
        run_to_block(3);
        {
            assert_eq!(
                IRMachine::machine_committee(&machine_id),
                OCMachineCommitteeList {
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

//  1. Commttee not submit hash
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
                OCMachineCommitteeList {
                    book_time: 2,
                    booked_committee: vec![committee3, committee2, committee4],
                    hashed_committee: vec![committee3, committee2],
                    confirm_start_time: 4320 + 2, // 360 * 12
                    status: OCVerifyStatus::SubmittingRaw,
                    ..Default::default()
                }
            );
        }

        // 两个委员会提交原始值
        let mut upload_info = CommitteeUploadInfo {
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
                IRMachine::pending_online_slash(0),
                OCPendingSlashInfo {
                    machine_id: machine_id.clone(),
                    inconsistent_committee: vec![],
                    unruly_committee: vec![committee4],
                    reward_committee: vec![committee3, committee2],
                    committee_stake: 1000 * ONE_DBC,

                    slash_time: 4 + 4320,
                    slash_exec_time: 4 + 4320 + 2880 * 2,

                    book_result: OCBookResultType::OnlineSucceed,
                    slash_result: OCSlashResult::Pending,
                    ..Default::default()
                }
            );
            assert_eq!(IRMachine::unhandled_online_slash(), vec![0]);
            assert_eq!(
                IRMachine::committee_machine(&committee4),
                OCCommitteeMachineList::default()
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

//  2. Commttee not submit raw
#[test]
fn committee_not_submit_raw_slash_works() {
    new_test_after_machine_distribute().execute_with(|| {
        //
    })
}

// 机器被拒绝后，惩罚矿工
// 机器上链需要验证，每台机器上链需要10000dbc作为保证金，当验证通过保证金解锁，
// 如果验证没通过保证金没收进入国库。
#[test]
fn machine_refused_slash_works() {
    new_test_after_machine_distribute().execute_with(|| {
        let stash = sr25519::Public::from(Sr25519Keyring::Ferdie);
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();

        let committee2 = sr25519::Public::from(Sr25519Keyring::Charlie);
        let committee3 = sr25519::Public::from(Sr25519Keyring::Dave);
        let committee4 = sr25519::Public::from(Sr25519Keyring::Eve);

        // 委员会添加机器Hash

        let hash1: [u8; 16] =
            hex::decode("cee14a520ba6a988c306aab9dc3794b1").unwrap().try_into().unwrap();
        let hash2: [u8; 16] =
            hex::decode("8c7e7ca563169689f1c789f8d4f510f8").unwrap().try_into().unwrap();
        let hash3: [u8; 16] =
            hex::decode("73af18cb31a2ebbea4eab9e9e519539e").unwrap().try_into().unwrap();

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
        assert_ok!(IRMachine::submit_confirm_hash(
            Origin::signed(committee4),
            machine_id.clone(),
            hash3
        ));

        // 委员会提交原始信息
        let mut upload_info = CommitteeUploadInfo {
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

        // 委员会添加机器原始值
        assert_ok!(IRMachine::submit_confirm_raw(Origin::signed(committee2), upload_info.clone()));
        upload_info.rand_str = "abcdefg2".as_bytes().to_vec();
        assert_ok!(IRMachine::submit_confirm_raw(Origin::signed(committee3), upload_info.clone()));
        upload_info.rand_str = "abcdefg3".as_bytes().to_vec();
        assert_ok!(IRMachine::submit_confirm_raw(Origin::signed(committee4), upload_info.clone()));

        run_to_block(4);
        {
            assert_eq!(
                IRMachine::live_machines(),
                IRLiveMachine { refused_machine: vec![machine_id.clone()], ..Default::default() }
            );
            let machine_info = IRMachine::machines_info(&machine_id);
            // MachineInfo 被删除
            assert_eq!(machine_info.machine_status, MachineStatus::AddingCustomizeInfo);
            assert_eq!(IRMachine::stash_machines(&stash), StashMachine::default());

            // 当机器审核通过，应该解锁保证金
            assert_eq!(Balances::free_balance(stash), INIT_BALANCE - 10000 * ONE_DBC);
            assert_eq!(Balances::reserved_balance(stash), 10000 * ONE_DBC);

            // 检查惩罚
            assert_eq!(
                IRMachine::pending_online_slash(0),
                OCPendingSlashInfo {
                    machine_id: machine_id.clone(),
                    machine_stash: stash,
                    stash_slash_amount: 10000 * ONE_DBC,
                    committee_stake: 1000 * ONE_DBC,

                    reward_committee: vec![committee3, committee2, committee4],

                    slash_time: 4,
                    slash_exec_time: 4 + 2880 * 2,

                    book_result: OCBookResultType::OnlineRefused,
                    slash_result: OCSlashResult::Pending,
                    ..Default::default()
                }
            );
        }

        run_to_block(4 + 2880 * 2 + 1);
        {
            assert_eq!(Balances::free_balance(stash), INIT_BALANCE - 10000 * ONE_DBC);
            assert_eq!(Balances::reserved_balance(stash), 0);

            assert_eq!(Balances::free_balance(committee2), INIT_BALANCE - 20000 * ONE_DBC);
            assert_eq!(Balances::free_balance(committee3), INIT_BALANCE - 20000 * ONE_DBC);
            // 为controller，多支付10 DBC
            assert_eq!(
                Balances::free_balance(committee4),
                INIT_BALANCE - 20000 * ONE_DBC - 10 * ONE_DBC
            );

            assert_eq!(Balances::reserved_balance(committee2), 20000 * ONE_DBC);
            assert_eq!(Balances::reserved_balance(committee3), 20000 * ONE_DBC);
            assert_eq!(Balances::reserved_balance(committee4), 20000 * ONE_DBC);
        }
    })
}
