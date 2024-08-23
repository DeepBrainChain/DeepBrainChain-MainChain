use super::super::mock::{TerminatingRental as IRMachine, *};
use crate::IRCommitteeOnlineOps;
use committee::CommitteeStakeInfo;
use dbc_support::{
    live_machine::LiveMachine,
    machine_type::{CommitteeUploadInfo, Latitude, Longitude, MachineStatus, StakerCustomizeInfo},
    verify_online::{
        OCCommitteeMachineList, OCMachineCommitteeList, OCMachineStatus as VerifyMachineStatus,
        OCVerifyStatus, StashMachine,
    },
    ONE_DAY, ONE_HOUR,
};
use frame_support::assert_ok;
use std::convert::TryInto;

pub fn new_test_with_machine_bonding_ext() -> sp_io::TestExternalities {
    let mut ext = new_test_with_init_params_ext();
    ext.execute_with(|| {
        let stash = sr25519::Public::from(Sr25519Keyring::Ferdie);
        let controller = sr25519::Public::from(Sr25519Keyring::Eve);

        assert_ok!(IRMachine::set_controller(RuntimeOrigin::signed(stash), controller));
        assert_ok!(IRMachine::gen_server_room(RuntimeOrigin::signed(controller)));

        // Bob pubkey
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();
        let msg = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48\
                   5CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL";
        let sig = "b4084f70730b183127e9db78c6d8dcf79039f23466cd1ee8b536c40c3027a83d\
                   ab040be4ed2db57b67eaac406817a69ce72a13f8ac11ba460e15d318b1504481";

        assert_ok!(IRMachine::bond_machine(
            RuntimeOrigin::signed(controller),
            machine_id.clone(),
            msg.as_bytes().to_vec(),
            hex::decode(sig).unwrap()
        ));

        assert_ok!(IRMachine::gen_server_room(RuntimeOrigin::signed(controller)));
        let server_rooms = IRMachine::stash_server_rooms(stash);

        assert_ok!(IRMachine::add_machine_info(
            RuntimeOrigin::signed(controller),
            machine_id.clone(),
            StakerCustomizeInfo {
                server_room: server_rooms[0],
                upload_net: 100,
                download_net: 100,
                longitude: Longitude::East(1157894),
                latitude: Latitude::North(235678),
                telecom_operators: vec!["China Unicom".into()],
                is_bare_machine: false
            }
        ));
    });
    ext
}

#[test]
fn verify_machine_works() {
    new_test_with_machine_bonding_ext().execute_with(|| {
        let stash = sr25519::Public::from(Sr25519Keyring::Ferdie);
        let _controller = sr25519::Public::from(Sr25519Keyring::Eve);
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();

        let committee1 = sr25519::Public::from(Sr25519Keyring::Alice);
        let _committee2 = sr25519::Public::from(Sr25519Keyring::Charlie);
        let committee3 = sr25519::Public::from(Sr25519Keyring::Dave);
        let committee4 = sr25519::Public::from(Sr25519Keyring::Eve);

        let committee1_box_pubkey =
            hex::decode("ff3033c763f71bc51f372c1dc5095accc26880e138df84cac13c46bfd7dbd74f")
                .unwrap()
                .try_into()
                .unwrap();

        run_to_block(3);
        // 自动派单
        // - Writes: CommitteeUsedStake, MachineCommittee, CommitteeMachine, CommitteeOps,
        // LiveMachines, MachinesInfo
        {
            assert_eq!(
                Committee::committee_stake(&committee1),
                CommitteeStakeInfo {
                    box_pubkey: committee1_box_pubkey,
                    staked_amount: 20000 * ONE_DBC,
                    used_stake: 1000 * ONE_DBC,
                    ..Default::default()
                }
            );
            assert_eq!(
                IRMachine::machine_committee(&machine_id),
                OCMachineCommitteeList {
                    book_time: 2,
                    booked_committee: vec![committee3, committee1, committee4],
                    confirm_start_time: 2 + 4320,
                    status: OCVerifyStatus::SubmittingHash,
                    hashed_committee: vec![],
                    confirmed_committee: vec![],
                    onlined_committee: vec![]
                }
            );
            assert_eq!(
                IRMachine::committee_machine(&committee1),
                OCCommitteeMachineList {
                    booked_machine: vec![machine_id.clone()],
                    ..Default::default()
                }
            );
            assert_eq!(
                IRMachine::committee_online_ops(&committee1, &machine_id),
                IRCommitteeOnlineOps {
                    staked_dbc: 1000 * ONE_DBC,
                    verify_time: vec![2, 1442, 2882], // 2 + 320 * 3
                    machine_status: VerifyMachineStatus::Booked,
                    ..Default::default()
                }
            );

            assert_eq!(
                IRMachine::live_machines(),
                LiveMachine { booked_machine: vec![machine_id.clone()], ..Default::default() }
            );
        }

        // 委员会添加机器Hash
        let hash1: [u8; 16] =
            hex::decode("53cf058dfa07ef517b2f28bccff88c2b").unwrap().try_into().unwrap();
        let hash2: [u8; 16] =
            hex::decode("3f775d3f4a144b94d6d551f6091a5126").unwrap().try_into().unwrap();
        let hash3: [u8; 16] =
            hex::decode("4983040157403addac94ca860ddbff7f").unwrap().try_into().unwrap();

        assert_ok!(IRMachine::submit_confirm_hash(
            RuntimeOrigin::signed(committee1),
            machine_id.clone(),
            hash1
        ));
        {
            // - Writes: CommitteeMachine, CommitteeOps, MachineSubmitedHash, MachineCommittee
            assert_eq!(IRMachine::machine_submited_hash(&machine_id), vec![hash1]);
            assert_eq!(
                IRMachine::machine_committee(&machine_id),
                OCMachineCommitteeList {
                    book_time: 2,
                    booked_committee: vec![committee3, committee1, committee4],
                    hashed_committee: vec![committee1],
                    confirm_start_time: 2 + ONE_DAY + 12 * ONE_HOUR,
                    status: OCVerifyStatus::SubmittingHash,
                    confirmed_committee: vec![],
                    onlined_committee: vec![]
                }
            );
            assert_eq!(
                IRMachine::committee_machine(&committee1),
                OCCommitteeMachineList {
                    hashed_machine: vec![machine_id.clone()],
                    ..Default::default()
                }
            );
            assert_eq!(
                IRMachine::committee_online_ops(&committee1, &machine_id),
                IRCommitteeOnlineOps {
                    staked_dbc: 1000 * ONE_DBC,
                    verify_time: vec![2, 1442, 2882],
                    confirm_hash: hash1,
                    hash_time: 4,
                    machine_status: VerifyMachineStatus::Hashed,
                    ..Default::default()
                }
            )
        }
        assert_ok!(IRMachine::submit_confirm_hash(
            RuntimeOrigin::signed(committee3),
            machine_id.clone(),
            hash2
        ));
        {
            assert_eq!(IRMachine::machine_submited_hash(&machine_id), vec![hash2, hash1]);
        }

        assert_ok!(IRMachine::submit_confirm_hash(
            RuntimeOrigin::signed(committee4),
            machine_id.clone(),
            hash3
        ));
        {
            assert_eq!(IRMachine::machine_submited_hash(&machine_id), vec![hash2, hash3, hash1]);
            assert_eq!(
                IRMachine::machine_committee(&machine_id),
                OCMachineCommitteeList {
                    book_time: 2,
                    booked_committee: vec![committee3, committee1, committee4],
                    hashed_committee: vec![committee3, committee1, committee4],
                    confirm_start_time: 2 + ONE_DAY + 12 * ONE_HOUR,
                    status: OCVerifyStatus::SubmittingRaw,
                    confirmed_committee: vec![],
                    onlined_committee: vec![]
                }
            );
        }

        // 委员会提交原始信息
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

        // 委员会添加机器原始值
        assert_ok!(IRMachine::submit_confirm_raw(
            RuntimeOrigin::signed(committee1),
            upload_info.clone()
        ));
        {
            assert_eq!(
                IRMachine::machine_committee(&machine_id),
                OCMachineCommitteeList {
                    book_time: 2,
                    booked_committee: vec![committee3, committee1, committee4],
                    hashed_committee: vec![committee3, committee1, committee4],
                    confirm_start_time: 2 + ONE_DAY + 12 * ONE_HOUR,
                    confirmed_committee: vec![committee1],
                    status: OCVerifyStatus::SubmittingRaw,
                    onlined_committee: vec![]
                }
            );
            assert_eq!(
                IRMachine::committee_machine(&committee1),
                OCCommitteeMachineList {
                    confirmed_machine: vec![machine_id.clone()],
                    ..Default::default()
                }
            );
            assert_eq!(
                IRMachine::committee_online_ops(&committee1, &machine_id),
                IRCommitteeOnlineOps {
                    staked_dbc: 1000 * ONE_DBC,
                    verify_time: vec![2, 1442, 2882],
                    confirm_hash: hash1,
                    hash_time: 4,
                    confirm_time: 4,
                    machine_status: VerifyMachineStatus::Confirmed,
                    machine_info: CommitteeUploadInfo { rand_str: vec![], ..upload_info.clone() },
                }
            )
        }
        upload_info.rand_str = "abcdefg2".as_bytes().to_vec();
        assert_ok!(IRMachine::submit_confirm_raw(
            RuntimeOrigin::signed(committee3),
            upload_info.clone()
        ));
        upload_info.rand_str = "abcdefg3".as_bytes().to_vec();
        assert_ok!(IRMachine::submit_confirm_raw(RuntimeOrigin::signed(committee4), upload_info));

        {
            assert_eq!(
                IRMachine::machine_committee(&machine_id),
                OCMachineCommitteeList {
                    book_time: 2,
                    booked_committee: vec![committee3, committee1, committee4],
                    hashed_committee: vec![committee3, committee1, committee4],
                    confirm_start_time: 2 + ONE_DAY + 12 * ONE_HOUR,
                    confirmed_committee: vec![committee3, committee1, committee4],
                    status: OCVerifyStatus::Summarizing,
                    onlined_committee: vec![]
                }
            );
        }

        run_to_block(4);
        {
            // Summary:
            //
            // - Writes: StashTotalStake, MachinesInfo, LiveMachines, StashMachines
            //
            // - Writes: MachineCommittee, CommitteeMachine, CommitteeStake
            // CommitteeOps, MachineSubmitedHash, CommitteeMachine
            assert_eq!(
                IRMachine::live_machines(),
                LiveMachine { online_machine: vec![machine_id.clone()], ..Default::default() }
            );

            let machine_info = IRMachine::machines_info(&machine_id).unwrap();
            assert_eq!(machine_info.machine_status, MachineStatus::Online);
            assert_eq!(machine_info.reward_committee, vec![committee3, committee1, committee4]);

            assert_eq!(
                IRMachine::stash_machines(&stash),
                StashMachine {
                    total_machine: vec![machine_id.clone()],
                    online_machine: vec![machine_id.clone()],
                    total_calc_points: 119780,
                    total_gpu_num: 8,
                    total_rented_gpu: 0,
                    total_rent_fee: 0,
                    ..Default::default()
                }
            );

            // 当机器审核通过，应该解锁保证金
            assert_eq!(Balances::free_balance(stash), INIT_BALANCE);
            assert_eq!(Balances::reserved_balance(stash), 0);

            // - Writes: CommitteeStake
            assert_eq!(
                IRMachine::machine_committee(&machine_id),
                OCMachineCommitteeList {
                    book_time: 2,
                    booked_committee: vec![committee3, committee1, committee4],
                    hashed_committee: vec![committee3, committee1, committee4],
                    confirm_start_time: 2 + ONE_DAY + 12 * ONE_HOUR,
                    confirmed_committee: vec![committee3, committee1, committee4],
                    status: OCVerifyStatus::Finished,
                    onlined_committee: vec![committee3, committee1, committee4],
                }
            );

            assert_eq!(
                <crate::CommitteeOnlineOps<TestRuntime>>::contains_key(committee1, &machine_id),
                false
            );
            assert_eq!(<crate::MachineSubmitedHash<TestRuntime>>::contains_key(&machine_id), false);
            assert_eq!(
                IRMachine::committee_machine(committee1),
                OCCommitteeMachineList {
                    online_machine: vec![machine_id.clone()],
                    ..Default::default()
                }
            );

            assert_eq!(
                Committee::committee_stake(&committee1),
                CommitteeStakeInfo {
                    box_pubkey: committee1_box_pubkey,
                    staked_amount: 20000 * ONE_DBC,
                    used_stake: 0,
                    ..Default::default()
                }
            );
        }
    })
}
