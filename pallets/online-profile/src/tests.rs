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
fn random_num_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        // let a = OnlineProfile::update_nonce();
        // assert_eq!(a, 1u64.encode());

        assert_eq!(OnlineProfile::rand_nonce(), 0);
        assert_eq!(OnlineProfile::random_num(12), 0);
        assert_eq!(OnlineProfile::rand_nonce(), 1);

        System::set_block_number(2);
        assert_eq!(OnlineProfile::random_num(12), 0);

        System::set_block_number(3);
        assert_eq!(OnlineProfile::random_num(13), 0);

        assert_eq!(OnlineProfile::random_num(14), 0);
    });
}
