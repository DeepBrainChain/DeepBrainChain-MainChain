use crate::{mock::*, Error};
use frame_support::{assert_noop, assert_ok};

#[test]
fn it_works_for_default_value() {
    new_test_ext().execute_with(|| {
        assert_ok!(LeaseCommittee::set_min_stake(
            RawOrigin::Root.into(),
            10u32.into()
        ));

        // // Dispatch a signed extrinsic.
        // assert_ok!(TemplateModule::do_something(Origin::signed(1), 42));
        // // Read pallet storage and assert an expected result.
        // assert_eq!(TemplateModule::something(), Some(42));
    });
}
