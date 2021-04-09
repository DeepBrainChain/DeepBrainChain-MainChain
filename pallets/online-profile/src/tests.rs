use crate::mock::*;
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
        assert_eq!(OnlineProfile::random_num(12), 5);
        assert_eq!(OnlineProfile::random_num(12), 5);
    });
}
