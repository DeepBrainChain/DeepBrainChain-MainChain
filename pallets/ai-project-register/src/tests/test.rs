use super::super::{mock::*, *};
use crate::mock::new_test_ext;
use dbc_support::{
    rental_type::{MachineGPUOrder, MachineRenterRentedOrderDetail, RentOrderDetail, RentStatus},
    traits::MachineInfoTrait,
};
use frame_support::{assert_err, assert_ok, traits::Currency};
use rent_machine::{
    Error as RentMachineError, MachineRentOrder, MachineRenterRentedOrders, RentInfo,
};

use dbc_support::{machine_info::MachineInfo, machine_type::MachineStatus};
use online_profile::MachinesInfo;
use sp_core::{sr25519, Pair};
pub use sp_keyring::sr25519::Keyring as Sr25519Keyring;

type BalanceOf<Test> = <<Test as rent_machine::Config>::Currency as Currency<
    <Test as frame_system::Config>::AccountId,
>>::Balance;

#[test]
fn test_add_machine_registered_project_should_work() {
    use sp_core::Pair;
    let _ = env_logger::builder().is_test(true).try_init();

    new_test_ext().execute_with(|| {
        let staker = sr25519::Public::from(Sr25519Keyring::Alice);
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();
        let fake_machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26xxx"
            .as_bytes()
            .to_vec();
        let project_name = "dgc".as_bytes().to_vec();
        let project_name1 = "dgc1".as_bytes().to_vec();
        let project_name2 = "dgc2".as_bytes().to_vec();
        let project_name3 = "dgc3".as_bytes().to_vec();

        let alice = sp_core::sr25519::Pair::from_string("//Alice", None).unwrap();
        let msg: Vec<u8> = b"The actual message".to_vec();
        let sig = alice.sign(&msg[..]);

        assert_err!(
            AiProjectRegister::add_machine_registered_project(
                msg.clone(),
                sig.clone(),
                alice.public(),
                machine_id.clone(),
                project_name.clone().clone()
            ),
            RentMachineError::<Test>::MachineNotRented.as_str()
        );

        let rent_info: RentOrderDetail<
            <Test as frame_system::Config>::AccountId,
            <Test as frame_system::Config>::BlockNumber,
            BalanceOf<Test>,
        > = RentOrderDetail {
            machine_id: machine_id.clone(),
            renter: sr25519::Public::from(Sr25519Keyring::One),
            rent_start: 1,
            confirm_rent: 1,
            rent_end: 1,
            stake_amount: 1,
            rent_status: Default::default(),
            gpu_num: 1,
            gpu_index: vec![],
        };
        RentInfo::<Test>::insert(1, rent_info);

        let order: MachineGPUOrder = MachineGPUOrder { rent_order: vec![1], used_gpu: vec![0] };

        MachineRentOrder::<Test>::insert(machine_id.clone(), order);

        assert_err!(
            AiProjectRegister::add_machine_registered_project(
                msg.clone(),
                sig.clone(),
                alice.public(),
                machine_id.clone(),
                project_name.clone()
            ),
            RentMachineError::<Test>::MachineNotRented.as_str()
        );

        RentInfo::<Test>::remove(1);
        MachineRentOrder::<Test>::remove(machine_id.clone());

        let rent_info: RentOrderDetail<
            <Test as frame_system::Config>::AccountId,
            <Test as frame_system::Config>::BlockNumber,
            BalanceOf<Test>,
        > = RentOrderDetail {
            machine_id: machine_id.clone(),
            renter: sr25519::Public::from(Sr25519Keyring::Alice),
            rent_start: 1,
            confirm_rent: 1,
            rent_end: 1,
            stake_amount: 1,
            rent_status: Default::default(),
            gpu_num: 1,
            gpu_index: vec![],
        };
        RentInfo::<Test>::insert(1, rent_info);
        let order: MachineGPUOrder = MachineGPUOrder { rent_order: vec![1], used_gpu: vec![0] };

        MachineRentOrder::<Test>::insert(machine_id.clone(), order);

        assert_err!(
            AiProjectRegister::add_machine_registered_project(
                msg.clone(),
                sig.clone(),
                alice.public(),
                machine_id.clone(),
                project_name.clone()
            ),
            RentMachineError::<Test>::MachineNotRented.as_str()
        );

        let rent_info_renting: RentOrderDetail<
            <Test as frame_system::Config>::AccountId,
            <Test as frame_system::Config>::BlockNumber,
            BalanceOf<Test>,
        > = RentOrderDetail {
            machine_id: machine_id.clone(),
            renter: sr25519::Public::from(Sr25519Keyring::Alice),
            rent_start: 1,
            confirm_rent: 1,
            rent_end: 1,
            stake_amount: 1,
            rent_status: RentStatus::Renting,
            gpu_num: 1,
            gpu_index: vec![],
        };
        RentInfo::<Test>::insert(2, rent_info_renting);

        let order: MachineGPUOrder = MachineGPUOrder { rent_order: vec![2], used_gpu: vec![0] };

        MachineRentOrder::<Test>::insert(machine_id.clone(), order);
        assert_err!(
            AiProjectRegister::add_machine_registered_project(
                msg.clone(),
                sig.clone(),
                alice.public(),
                fake_machine_id.clone(),
                project_name.clone()
            ),
            RentMachineError::<Test>::MachineNotRented.as_str()
        );
        assert_ok!(AiProjectRegister::add_machine_registered_project(
            msg.clone(),
            sig.clone(),
            alice.public(),
            machine_id.clone(),
            project_name.clone()
        ));

        assert_eq!(
            AiProjectRegister::is_registered(machine_id.clone(), project_name.clone()),
            true
        );
        assert_ok!(AiProjectRegister::add_machine_registered_project(
            msg.clone(),
            sig.clone(),
            alice.public(),
            machine_id.clone(),
            project_name.clone()
        ));

        assert_ok!(AiProjectRegister::add_machine_registered_project(
            msg.clone(),
            sig.clone(),
            alice.public(),
            machine_id.clone(),
            project_name1.clone()
        ));
        assert_eq!(
            AiProjectRegister::is_registered(machine_id.clone(), project_name1.clone()),
            true
        );

        assert_ok!(AiProjectRegister::add_machine_registered_project(
            msg.clone(),
            sig.clone(),
            alice.public(),
            machine_id.clone(),
            project_name2.clone()
        ));

        assert_eq!(
            AiProjectRegister::is_registered(machine_id.clone(), project_name2.clone()),
            true
        );

        assert_eq!(
            AiProjectRegister::registered_info_to_owner(machine_id.clone(), project_name2.clone())
                .unwrap()
                .eq(&staker),
            true
        );

        assert_err!(
            AiProjectRegister::add_machine_registered_project(
                msg.clone(),
                sig.clone(),
                alice.public(),
                machine_id.clone(),
                project_name3.clone()
            ),
            Error::<Test>::OverLimitPerMachineIdCanRegister.as_str()
        );
    });
}
#[test]
fn test_remove_machine_registered_project_should_work() {
    new_test_ext().execute_with(|| {
        System::set_block_number(10);

        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();
        let project_name = "dgc".as_bytes().to_vec();
        let project_name1 = "dgc1".as_bytes().to_vec();
        let project_name2 = "dgc2".as_bytes().to_vec();
        let project_name3 = "dgc3".as_bytes().to_vec();

        let alice = sp_core::sr25519::Pair::from_string("//Alice", None).unwrap();
        let msg: Vec<u8> = b"The actual message".to_vec();
        let sig = alice.sign(&msg[..]);

        let rent_info_renting: RentOrderDetail<
            <Test as frame_system::Config>::AccountId,
            <Test as frame_system::Config>::BlockNumber,
            BalanceOf<Test>,
        > = RentOrderDetail {
            machine_id: machine_id.clone(),
            renter: sr25519::Public::from(Sr25519Keyring::Alice),
            rent_start: 1,
            confirm_rent: 1,
            rent_end: 1,
            stake_amount: 1,
            rent_status: RentStatus::Renting,
            gpu_num: 1,
            gpu_index: vec![],
        };
        RentInfo::<Test>::insert(2, rent_info_renting);
        let order: MachineGPUOrder = MachineGPUOrder { rent_order: vec![2], used_gpu: vec![0] };

        MachineRentOrder::<Test>::insert(machine_id.clone(), order);

        assert_err!(
            AiProjectRegister::remove_machine_registered_project(
                msg.clone(),
                sig.clone(),
                alice.public(),
                machine_id.clone(),
                project_name.clone()
            ),
            Error::<Test>::MachineNotRegistered.as_str()
        );
        assert_ok!(AiProjectRegister::add_machine_registered_project(
            msg.clone(),
            sig.clone(),
            alice.public(),
            machine_id.clone(),
            project_name1.clone()
        ));
        assert_ok!(AiProjectRegister::add_machine_registered_project(
            msg.clone(),
            sig.clone(),
            alice.public(),
            machine_id.clone(),
            project_name2.clone()
        ));
        assert_eq!(
            AiProjectRegister::machine_id_to_ai_project_name(machine_id.clone()),
            vec![project_name1.clone(), project_name2.clone()]
        );

        assert_ok!(AiProjectRegister::add_machine_registered_project(
            msg.clone(),
            sig.clone(),
            alice.public(),
            machine_id.clone(),
            project_name3.clone()
        ));

        assert_eq!(
            AiProjectRegister::machine_id_to_ai_project_name(machine_id.clone()),
            vec![project_name1.clone(), project_name2.clone(), project_name3.clone()]
        );
        assert_err!(
            AiProjectRegister::remove_machine_registered_project(
                msg.clone(),
                sig.clone(),
                alice.public(),
                machine_id.clone(),
                project_name.clone()
            ),
            Error::<Test>::MachineNotRegistered.as_str()
        );
        assert_ok!(AiProjectRegister::remove_machine_registered_project(
            msg.clone(),
            sig.clone(),
            alice.public(),
            machine_id.clone(),
            project_name1.clone()
        ));
        assert_eq!(
            AiProjectRegister::machine_id_to_ai_project_name(machine_id.clone()),
            vec![project_name2.clone(), project_name3.clone()]
        );
        assert_eq!(
            AiProjectRegister::registered_info_to_owner(machine_id.clone(), project_name1.clone())
                .is_none(),
            true
        );
        assert_eq!(
            AiProjectRegister::projec_machine_to_unregistered_times(project_name1, machine_id),
            10
        );
    });
}

#[test]
fn sig_verify_should_works() {
    use sp_core::Pair;
    let _ = env_logger::builder().is_test(true).try_init();

    new_test_ext().execute_with(|| {
        let alice = sp_core::sr25519::Pair::from_string("//Alice", None).unwrap();
        let msg: Vec<u8> = b"The actual message".to_vec();
        let sig = alice.sign(&msg[..]);


        // Works as expected - no magic involved.
        assert_eq!(verify_signature(msg.clone(), sig.clone(), alice.public()),true);
        // Signature on "The actual message" by Alice via PolkadotJS.
        let alice = sp_core::sr25519::Pair::from_string("//Alice", None).unwrap();
        // Signature on "The actual message" by Alice via PolkadotJS.
        let origin_sig = b"860ab35af395c6cc989b0498269d26c13d488431f8ceac89ed82744eb84361162ce7f6c817575e46c07287f000397a0c3d5521577ac63e20ce1d0b3ab158cd88";
        let sig = hex::decode(origin_sig).unwrap();
        println!("sig size: {:?}",sig.len());

        // let a: = sp_core::H256::from(alice.public().as_bytes());
        let sig2 = sp_core::sr25519::Signature::from_slice(&sig[..]).unwrap();
        // This will not work since it's missing the wrapping:
        let msg: Vec<u8> = b"The actual message".to_vec();
        assert_eq!(verify_signature(msg,sig2.clone(), alice.public()),false);

        // This will work since it's wrapped:
        let msg: Vec<u8> = b"<Bytes>The actual message</Bytes>".to_vec();

        assert_eq!(verify_signature(msg, sig2, alice.public()),true);


        let msg: Vec<u8> = b"123".to_vec();
        let sig_str = "5cae2fcdb2c088cc288dea283a10ad260a5e6df8dc4c07b00ad086967e354850cbbec859aaefa5ddc277d6b1c790c9be43b56160965a7dafdf10abbf7d976189";
        let pub_key_str = "34e9bdb4c0107a249c44515441acbda4b9d0f03db123241a54711d2a8ed6ce51";


        let sig = hex::decode(sig_str.as_bytes()).unwrap();

        let mut b = [0u8; 64];
        b.copy_from_slice(&sig[..]);
        let sig = sp_core::sr25519::Signature(b);


        let pub_key = hex::decode(pub_key_str.as_bytes()).unwrap();

        let mut b = [0u8; 32];
        b.copy_from_slice(&pub_key[..]);
        let pub_key = sp_core::sr25519::Public(b);

        println!("sig: {:?}, pub_key: {:?}", sig, pub_key);
        assert_eq!(verify_signature(msg, sig,pub_key),true);
    })
}
#[test]
fn test_account_id_should_works() {
    use sp_core::Pair;
    let _ = env_logger::builder().is_test(true).try_init();

    new_test_ext().execute_with(|| {
        let alice = sp_core::sr25519::Pair::from_string("//Alice", None).unwrap();

        let r = account_id::<Test>(alice.public()).unwrap();
        assert_eq!(r, alice.public());
    });
}

#[test]
fn test_get_machine_valid_stake_duration_should_works() {
    use sp_core::Pair;
    let _ = env_logger::builder().is_test(true).try_init();

    new_test_ext().execute_with(|| {
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();

        let alice = sp_core::sr25519::Pair::from_string("//Alice", None).unwrap();
        let msg: Vec<u8> = b"The actual message".to_vec();
        let sig = alice.sign(&msg[..]);

        let rent_info_renting: RentOrderDetail<
            <Test as frame_system::Config>::AccountId,
            <Test as frame_system::Config>::BlockNumber,
            BalanceOf<Test>,
        > = RentOrderDetail {
            machine_id: machine_id.clone(),
            renter: sr25519::Public::from(Sr25519Keyring::Alice),
            rent_start: 1,
            confirm_rent: 1,
            rent_end: 1,
            stake_amount: 1,
            rent_status: RentStatus::Renting,
            gpu_num: 1,
            gpu_index: vec![],
        };

        RentInfo::<Test>::insert(1, rent_info_renting.clone());

        let order: MachineRenterRentedOrderDetail<<Test as frame_system::Config>::BlockNumber> =
            MachineRenterRentedOrderDetail { rent_id: 1, rent_start: 1, rent_end: 2 };

        MachineRenterRentedOrders::<Test>::insert(
            machine_id.clone(),
            rent_info_renting.renter,
            vec![order],
        );

        let machine_info: MachineInfo<
            <Test as frame_system::Config>::AccountId,
            <Test as frame_system::Config>::BlockNumber,
            BalanceOf<Test>,
        > = MachineInfo {
            controller: sr25519::Public::from(Sr25519Keyring::Alice),
            machine_stash: sr25519::Public::from(Sr25519Keyring::Bob),
            renters: vec![sr25519::Public::from(Sr25519Keyring::Alice)],
            last_machine_restake: 1,
            bonding_height: 1,
            online_height: 1,
            last_online_height: 1,
            init_stake_per_gpu: 1,
            stake_amount: 1,
            machine_status: MachineStatus::Rented,
            total_rented_duration: 1,
            total_rented_times: 0,
            total_rent_fee: 1,
            total_burn_fee: 1,
            machine_info_detail: Default::default(),
            reward_committee: vec![],
            reward_deadline: 0,
        };

        MachinesInfo::<Test>::insert(machine_id.clone(), machine_info.clone());

        System::set_block_number(10);
        let r = RentMachine::get_machine_valid_stake_duration(0, 0, 0, machine_id.clone());
        assert_eq!(r.unwrap(), 1);

        let rent_info_renting: RentOrderDetail<
            <Test as frame_system::Config>::AccountId,
            <Test as frame_system::Config>::BlockNumber,
            BalanceOf<Test>,
        > = RentOrderDetail {
            machine_id: machine_id.clone(),
            renter: sr25519::Public::from(Sr25519Keyring::Alice),
            rent_start: 1,
            confirm_rent: 1,
            rent_end: 20,
            stake_amount: 1,
            rent_status: RentStatus::Renting,
            gpu_num: 1,
            gpu_index: vec![],
        };
        RentInfo::<Test>::insert(2, rent_info_renting.clone());
        let order: MachineRenterRentedOrderDetail<<Test as frame_system::Config>::BlockNumber> =
            MachineRenterRentedOrderDetail { rent_id: 2, rent_start: 1, rent_end: 20 };
        MachineRenterRentedOrders::<Test>::insert(
            machine_id.clone(),
            rent_info_renting.renter,
            vec![order],
        );

        let r = RentMachine::get_machine_valid_stake_duration(0, 0, 0, machine_id);
        assert_eq!(r.unwrap(), 9);
    });
}
