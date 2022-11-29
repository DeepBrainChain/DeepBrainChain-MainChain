use super::super::mock::{TerminatingRental as IRMachine, *};
use crate::{
    tests::test_verify_online::new_test_with_machine_bonding_ext, IRCommitteeUploadInfo,
    IRLiveMachine, IRMachineGPUOrder, IRMachineStatus, IRRentOrderDetail, IRRentStatus,
    IRStashMachine,
};
// use committee::CommitteeStakeInfo;
use frame_support::assert_ok;
use std::convert::TryInto;

pub fn new_test_with_machine_online_ext() -> sp_io::TestExternalities {
    let mut ext = new_test_with_machine_bonding_ext();
    ext.execute_with(|| {
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();

        let committee1 = sr25519::Public::from(Sr25519Keyring::Alice);
        let committee2 = sr25519::Public::from(Sr25519Keyring::Charlie);
        let _committee3 = sr25519::Public::from(Sr25519Keyring::Dave);
        let committee4 = sr25519::Public::from(Sr25519Keyring::Eve);

        // 委员会添加机器Hash
        let hash1: [u8; 16] =
            hex::decode("53cf058dfa07ef517b2f28bccff88c2b").unwrap().try_into().unwrap();
        let hash2: [u8; 16] =
            hex::decode("3f775d3f4a144b94d6d551f6091a5126").unwrap().try_into().unwrap();
        let hash3: [u8; 16] =
            hex::decode("4983040157403addac94ca860ddbff7f").unwrap().try_into().unwrap();

        run_to_block(3);

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

#[test]
fn rent_machine_works() {
    new_test_with_machine_online_ext().execute_with(|| {
        let stash = sr25519::Public::from(Sr25519Keyring::Ferdie);
        let _controller = sr25519::Public::from(Sr25519Keyring::Eve);
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();

        // 用户租用
        let renter1 = sr25519::Public::from(Sr25519Keyring::Alice);
        // let renter2 = sr25519::Public::from(Sr25519Keyring::Bob);
        assert_ok!(IRMachine::rent_machine(Origin::signed(renter1), machine_id.clone(), 8, 60));
        {
            // - Writes: MachineRentOrder, RentOrder, machine_status, UserRented, PendingRentEnding,
            // PendingConfirming, RenterTotalStake, FreeBalance(少10DBC)
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
            assert_eq!(
                Balances::free_balance(renter1),
                INIT_BALANCE - 10 * ONE_DBC - (1039756916666666666 + 20000 * ONE_DBC)
            );
            assert_eq!(IRMachine::renter_total_stake(renter1), 1039756916666666666);
            assert_eq!(
                IRMachine::machine_rent_order(&machine_id),
                IRMachineGPUOrder { rent_order: vec![0], used_gpu: vec![0, 1, 2, 3, 4, 5, 6, 7] }
            );

            let machine_info = IRMachine::machines_info(&machine_id);
            assert_eq!(machine_info.machine_status, IRMachineStatus::Rented);
        }

        assert_ok!(IRMachine::confirm_rent(Origin::signed(renter1), 0));
        {
            // - Writes: PendingConfirming, RentOrder, LiveMachine, MachineInfo, StashMachine

            // - Writes: PendingConfirming,
            assert_eq!(
                IRMachine::live_machines(),
                IRLiveMachine { rented_machine: vec![machine_id.clone()], ..Default::default() }
            );
            let machine_info = IRMachine::machines_info(&machine_id);
            assert_eq!(machine_info.total_rented_times, 1);
            assert_eq!(machine_info.renters, vec![renter1]);

            assert_eq!(
                IRMachine::stash_machines(stash),
                IRStashMachine {
                    total_machine: vec![machine_id.clone()],
                    online_machine: vec![machine_id.clone()],
                    total_calc_points: 119780,
                    total_rented_gpu: 8,
                    total_gpu_num: 8,
                    total_rent_fee: 0,
                }
            );

            // TODO: 当为空时，删除
            // assert_eq!(<crate::PendingConfirming<TestRuntime>>::contains_key(35), false);

            assert_eq!(
                IRMachine::rent_order(0),
                IRRentOrderDetail {
                    machine_id: machine_id.clone(),
                    renter: renter1,
                    rent_start: 5,
                    confirm_rent: 5,
                    rent_end: 65,
                    stake_amount: 1039756916666666666,
                    rent_status: IRRentStatus::Renting,
                    gpu_num: 8,
                    gpu_index: vec![0, 1, 2, 3, 4, 5, 6, 7],
                }
            );

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
            // - Writes: MachineRentedGPU, LiveMachines, MachinesInfo, StashMachine
            // 结束租用
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
