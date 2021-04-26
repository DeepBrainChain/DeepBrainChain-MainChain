use crate::mock::*;
use codec::Encode;
use frame_support::assert_ok;

#[test]
fn set_storage_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
    });
}

#[test]
#[rustfmt::skip]
fn bond_machine_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let alice: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Alice).into();
        let bob: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Bob).into();
        let charile: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Charlie).into();
        let dave: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Dave).into();
        let eve: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();

        assert_ok!(OnlineProfile::set_min_stake(RawOrigin::Root.into(), 500_0000u32.into()));
        assert_eq!(OnlineProfile::min_stake(), 500_0000);

        // let a = OnlineProfile::update_nonce();
        // assert_eq!(a, 1u64.encode());

        // assert_eq!(OnlineProfile::rand_nonce(), 0);
        // assert_eq!(OnlineProfile::random_num(12), 0);
        // assert_eq!(OnlineProfile::rand_nonce(), 1);

        // System::set_block_number(2);
        // assert_eq!(OnlineProfile::random_num(12), 0);

        // System::set_block_number(3);
        // assert_eq!(OnlineProfile::random_num(13), 0);

        // assert_eq!(OnlineProfile::random_num(14), 0);
    });
}

#[test]
fn unbond_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
    });
}
