use frame_support::{assert_err, assert_ok};
use frame_support::traits::Currency;
use sp_core::sr25519;
use crate::mock::new_test_ext;
use super::super::{mock::*, *};
pub use sp_keyring::{
    ed25519::Keyring as Ed25519Keyring, sr25519::Keyring as Sr25519Keyring, AccountKeyring,
};
use dbc_support::rental_type::RentOrderDetail;

use crate::{Error as Err};
use rent_machine::RentInfo;

type BalanceOf<Test> =
<<Test as rent_machine::Config>::Currency as Currency<<Test as frame_system::Config>::AccountId>>::Balance;

#[test]
fn test_add_machine_registered_project_should_work() {
    new_test_ext().execute_with(|| {
        let fake_staker = sr25519::Public::from(Sr25519Keyring::Two);
        let staker = sr25519::Public::from(Sr25519Keyring::One);
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();
        let fake_machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26xxx"
            .as_bytes()
            .to_vec();
        let project_name = "dgc"
            .as_bytes()
            .to_vec();
        let project_name1 = "dgc1"
            .as_bytes()
            .to_vec();
        let project_name2 = "dgc2"
            .as_bytes()
            .to_vec();
        let project_name3 = "dgc3"
            .as_bytes()
            .to_vec();

        assert_err!(AiProjectRegister::add_machine_registered_project(RuntimeOrigin::signed(staker),1,machine_id.clone(),project_name.clone().clone()),Err::<Test>::RentInfoNotFound);

        let  rent_info : RentOrderDetail<<Test as frame_system::Config>::AccountId, <Test as frame_system::Config>::BlockNumber, BalanceOf<Test>>= RentOrderDetail{
            machine_id:machine_id.clone(),
            renter: sr25519::Public::from(Sr25519Keyring::One),
            rent_start: 1,
            confirm_rent: 1,
            rent_end: 1,
            stake_amount: 1,
            rent_status: Default::default(),
            gpu_num: 1,
            gpu_index: vec![],
        };
        RentInfo::<Test>::insert(1,rent_info);

        assert_err!(AiProjectRegister::add_machine_registered_project(RuntimeOrigin::signed(fake_staker),1,machine_id.clone(),project_name.clone()),Err::<Test>::NotRentOwner);
        assert_err!(AiProjectRegister::add_machine_registered_project(RuntimeOrigin::signed(staker),1,machine_id.clone(),project_name.clone()),Err::<Test>::StatusNotRenting);


        let rent_info_renting : RentOrderDetail<<Test as frame_system::Config>::AccountId, <Test as frame_system::Config>::BlockNumber, BalanceOf<Test>>= RentOrderDetail{
            machine_id:machine_id.clone(),
            renter: sr25519::Public::from(Sr25519Keyring::One),
            rent_start: 1,
            confirm_rent: 1,
            rent_end: 1,
            stake_amount: 1,
            rent_status: RentStatus::Renting,
            gpu_num: 1,
            gpu_index: vec![],
        };
        RentInfo::<Test>::insert(2,rent_info_renting );

        assert_err!(AiProjectRegister::add_machine_registered_project(RuntimeOrigin::signed(staker),2,fake_machine_id.clone(),project_name.clone()),Err::<Test>::NotRentMachine);
        assert_ok!(AiProjectRegister::add_machine_registered_project(RuntimeOrigin::signed(staker),2,machine_id.clone(),project_name.clone()));
        assert_ok!(AiProjectRegister::add_machine_registered_project(RuntimeOrigin::signed(staker),2,machine_id.clone(),project_name.clone()));

        assert_ok!(AiProjectRegister::add_machine_registered_project(RuntimeOrigin::signed(staker),2,machine_id.clone(),project_name1.clone()));
        assert_ok!(AiProjectRegister::add_machine_registered_project(RuntimeOrigin::signed(staker),2,machine_id.clone(),project_name2.clone()));
        assert_err!(AiProjectRegister::add_machine_registered_project(RuntimeOrigin::signed(staker),2,machine_id.clone(),project_name3.clone()),Err::<Test>::OverMaxLimitPerMachineIdCanRegister);
    });
}
#[test]
fn test_remove_machine_registered_project_should_work() {
    new_test_ext().execute_with(|| {
        let fake_staker = sr25519::Public::from(Sr25519Keyring::Two);
        let staker = sr25519::Public::from(Sr25519Keyring::One);
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();
        let fake_machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26xxx"
            .as_bytes()
            .to_vec();
        let project_name = "dgc"
            .as_bytes()
            .to_vec();
        let project_name1 = "dgc1"
            .as_bytes()
            .to_vec();
        let project_name2 = "dgc2"
            .as_bytes()
            .to_vec();

    let rent_info_renting : RentOrderDetail<<Test as frame_system::Config>::AccountId, <Test as frame_system::Config>::BlockNumber, BalanceOf<Test>>= RentOrderDetail{
        machine_id:machine_id.clone(),
        renter: sr25519::Public::from(Sr25519Keyring::One),
        rent_start: 1,
        confirm_rent: 1,
        rent_end: 1,
        stake_amount: 1,
        rent_status: RentStatus::Renting,
        gpu_num: 1,
        gpu_index: vec![],
    };
    RentInfo::<Test>::insert(2,rent_info_renting );
    assert_err!(AiProjectRegister::remove_machine_registered_project(RuntimeOrigin::signed(staker),2,machine_id.clone(),project_name.clone()),Err::<Test>::NotRegistered);
    assert_ok!(AiProjectRegister::add_machine_registered_project(RuntimeOrigin::signed(staker),2,machine_id.clone(),project_name1.clone()));
    assert_ok!(AiProjectRegister::add_machine_registered_project(RuntimeOrigin::signed(staker),2,machine_id.clone(),project_name2.clone()));
    assert_eq!(AiProjectRegister::machine_id_to_ai_project_name(machine_id.clone()),vec![project_name1.clone(),project_name2.clone()]);
    assert_err!(AiProjectRegister::remove_machine_registered_project(RuntimeOrigin::signed(staker),2,machine_id.clone(),project_name.clone()),Err::<Test>::NotRegistered);
    assert_ok!(AiProjectRegister::remove_machine_registered_project(RuntimeOrigin::signed(staker),2,machine_id.clone(),project_name1.clone()));
    assert_eq!(AiProjectRegister::machine_id_to_ai_project_name(machine_id.clone()),vec![project_name2.clone()]);
    });
}