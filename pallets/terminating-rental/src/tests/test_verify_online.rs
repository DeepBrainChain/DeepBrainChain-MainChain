use crate::{
    IRCommitteeMachineList, IRCommitteeOps, IRCommitteeUploadInfo, IRLiveMachine,
    IRMachineCommitteeList, IRMachineInfo, IRMachineStatus, IRStakerCustomizeInfo, IRStashMachine,
    IRVerifyMachineStatus, IRVerifyStatus,
};

use super::super::mock::{TerminatingRental as IRMachine, *};
use frame_support::assert_ok;
use std::convert::TryInto;

pub fn new_test_with_machine_bonding_ext() -> sp_io::TestExternalities {
    let mut ext = new_test_with_init_params_ext();
    ext.execute_with(|| {
        let stash = sr25519::Public::from(Sr25519Keyring::Ferdie);
        let controller = sr25519::Public::from(Sr25519Keyring::Eve);

        assert_ok!(IRMachine::set_controller(Origin::signed(stash), controller));
        assert_ok!(IRMachine::gen_server_room(Origin::signed(controller)));

        // Bob pubkey
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();
        let msg = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48\
                   5CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL";
        let sig = "b4084f70730b183127e9db78c6d8dcf79039f23466cd1ee8b536c40c3027a83d\
                   ab040be4ed2db57b67eaac406817a69ce72a13f8ac11ba460e15d318b1504481";

        assert_ok!(IRMachine::bond_machine(
            Origin::signed(controller),
            machine_id.clone(),
            msg.as_bytes().to_vec(),
            hex::decode(sig).unwrap()
        ));

        assert_ok!(IRMachine::gen_server_room(Origin::signed(controller)));
        let server_rooms = IRMachine::stash_server_rooms(stash);

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
    });
    ext
}

#[test]
fn verify_machine_works() {
    new_test_with_machine_bonding_ext().execute_with(|| {
        let stash = sr25519::Public::from(Sr25519Keyring::Ferdie);
        let controller = sr25519::Public::from(Sr25519Keyring::Eve);
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();

        let committee1 = sr25519::Public::from(Sr25519Keyring::Alice);
        let committee2 = sr25519::Public::from(Sr25519Keyring::Charlie);
        let committee3 = sr25519::Public::from(Sr25519Keyring::Dave);
        let committee4 = sr25519::Public::from(Sr25519Keyring::Eve);

        run_to_block(3);
        // 自动派单
        {
            assert_eq!(
                IRMachine::machine_committee(&machine_id),
                IRMachineCommitteeList {
                    book_time: 2,
                    booked_committee: vec![committee2, committee1, committee4],
                    confirm_start_time: 2 + 4320, // 36h = 2880 + 1440
                    status: IRVerifyStatus::SubmittingHash,
                    ..Default::default()
                }
            );
            assert_eq!(
                IRMachine::committee_machine(&committee1),
                IRCommitteeMachineList {
                    booked_machine: vec![machine_id.clone()],
                    ..Default::default()
                }
            );
            assert_eq!(
                IRMachine::committee_ops(&committee1, &machine_id),
                IRCommitteeOps {
                    staked_dbc: 1000 * ONE_DBC,
                    verify_time: vec![962, 2402, 3842], // 2 + 320 * 3
                    machine_status: IRVerifyMachineStatus::Booked,
                    ..Default::default()
                }
            );

            assert_eq!(
                IRMachine::live_machines(),
                IRLiveMachine { booked_machine: vec![machine_id.clone()], ..Default::default() }
            );
            // TODO:
            // assert_eq!(IRMachine::machines_info(&machine_id), IRMachineInfo {
            // ..Default::default() });
        }

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
        {
            assert_eq!(IRMachine::machine_submited_hash(&machine_id), vec![hash1]);
            assert_eq!(
                IRMachine::machine_committee(&machine_id),
                IRMachineCommitteeList {
                    book_time: 2,
                    booked_committee: vec![committee2, committee1, committee4],
                    hashed_committee: vec![committee1,],
                    confirm_start_time: 2 + 2880 + 1440,
                    status: IRVerifyStatus::SubmittingHash,
                    ..Default::default()
                }
            );
            assert_eq!(
                IRMachine::committee_machine(&committee1),
                IRCommitteeMachineList {
                    hashed_machine: vec![machine_id.clone()],
                    ..Default::default()
                }
            );
            assert_eq!(
                IRMachine::committee_ops(&committee1, &machine_id),
                IRCommitteeOps {
                    staked_dbc: 1000 * ONE_DBC,
                    verify_time: vec![962, 2402, 3842],
                    confirm_hash: hash1,
                    hash_time: 4,
                    machine_status: IRVerifyMachineStatus::Hashed,
                    ..Default::default()
                }
            )
        }
        assert_ok!(IRMachine::submit_confirm_hash(
            Origin::signed(committee2),
            machine_id.clone(),
            hash2
        ));
        {
            assert_eq!(IRMachine::machine_submited_hash(&machine_id), vec![hash2, hash1]);
        }

        assert_ok!(IRMachine::submit_confirm_hash(
            Origin::signed(committee4),
            machine_id.clone(),
            hash3
        ));
        {
            assert_eq!(IRMachine::machine_submited_hash(&machine_id), vec![hash2, hash3, hash1]);
        }

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
        {
            assert_eq!(
                IRMachine::machine_committee(&machine_id),
                IRMachineCommitteeList {
                    book_time: 2,
                    booked_committee: vec![committee2, committee1, committee4],
                    hashed_committee: vec![committee2, committee1, committee4],
                    confirm_start_time: 2 + 2880 + 1440,
                    confirmed_committee: vec![committee1],
                    status: IRVerifyStatus::SubmittingRaw,
                    ..Default::default()
                }
            );
            assert_eq!(
                IRMachine::committee_machine(&committee1),
                IRCommitteeMachineList {
                    confirmed_machine: vec![machine_id.clone()],
                    ..Default::default()
                }
            );
            assert_eq!(
                IRMachine::committee_ops(&committee1, &machine_id),
                IRCommitteeOps {
                    staked_dbc: 1000 * ONE_DBC,
                    verify_time: vec![962, 2402, 3842],
                    confirm_hash: hash1,
                    hash_time: 4,
                    confirm_time: 4,
                    machine_status: IRVerifyMachineStatus::Confirmed,
                    machine_info: IRCommitteeUploadInfo { rand_str: vec![], ..upload_info.clone() },
                }
            )
        }
        upload_info.rand_str = "abcdefg2".as_bytes().to_vec();
        assert_ok!(IRMachine::submit_confirm_raw(Origin::signed(committee2), upload_info.clone()));
        upload_info.rand_str = "abcdefg3".as_bytes().to_vec();
        assert_ok!(IRMachine::submit_confirm_raw(Origin::signed(committee4), upload_info));

        run_to_block(4);
        {
            assert_eq!(
                IRMachine::live_machines(),
                IRLiveMachine { online_machine: vec![machine_id.clone()], ..Default::default() }
            );
            let machine_info = IRMachine::machines_info(&machine_id);
            assert_eq!(machine_info.machine_status, IRMachineStatus::Online);
            assert_eq!(
                IRMachine::stash_machines(&stash),
                IRStashMachine {
                    total_machine: vec![machine_id.clone()],
                    online_machine: vec![machine_id.clone()],
                    total_calc_points: 119780,
                    total_gpu_num: 8,
                    total_rented_gpu: 0,
                    total_rent_fee: 0,
                }
            );

            // 当机器审核通过，应该解锁保证金
            assert_eq!(Balances::free_balance(stash), INIT_BALANCE);
            assert_eq!(Balances::reserved_balance(stash), 0);
        }

        // 用户租用
        let renter1 = sr25519::Public::from(Sr25519Keyring::Alice);
        // let renter2 = sr25519::Public::from(Sr25519Keyring::Bob);
        assert_ok!(IRMachine::rent_machine(Origin::signed(renter1), machine_id.clone(), 8, 60));
        {
            assert_eq!(TerminatingRental::user_rented(renter1), vec![0]);
            assert_eq!(
                TerminatingRental::rent_order(0),
                crate::IRRentOrderDetail {
                    machine_id: machine_id.clone(),
                    renter: renter1,
                    rent_start: 5,
                    confirm_rent: 0,
                    rent_end: 5 + 60,
                    // 租金: 119780 / 1000 * 5000000 / 12000 * (60 / 2880)
                    stake_amount: 1039756916666666666,
                    rent_status: crate::IRRentStatus::WaitingVerifying,
                    gpu_num: 8,
                    gpu_index: vec![0, 1, 2, 3, 4, 5, 6, 7]
                }
            );
            assert_eq!(TerminatingRental::pending_rent_ending(5 + 60), vec![0]);
            assert_eq!(TerminatingRental::pending_confirming(5 + 30), vec![0]);
            assert_eq!(Balances::reserved_balance(renter1), 1039756916666666666 + 20000 * ONE_DBC);
        }

        assert_ok!(IRMachine::confirm_rent(Origin::signed(renter1), 0));
        {
            assert_eq!(Balances::reserved_balance(renter1), 1039756916666666666 + 20000 * ONE_DBC);
            assert_eq!(
                Balances::free_balance(renter1),
                INIT_BALANCE - (1039756916666666666 + 20000 * ONE_DBC + 10 * ONE_DBC)
            );

            assert_eq!(Balances::reserved_balance(stash), 0);
            assert_eq!(Balances::free_balance(stash), INIT_BALANCE);
        }

        run_to_block(100);
        {
            assert_eq!(Balances::reserved_balance(renter1), 20000 * ONE_DBC);
            assert_eq!(
                Balances::free_balance(renter1),
                INIT_BALANCE - 1039756916666666666 - 20000 * ONE_DBC - 10 * ONE_DBC
            );

            // 租金被质押
            assert_eq!(Balances::free_balance(stash), INIT_BALANCE);
            assert_eq!(Balances::reserved_balance(stash), 1039756916666666666);
        }
        // 这时候质押的金额应该转给stash账户,
        // 如果stash的押金够则转到stash的free，否则转到staked
    })
}
