use crate::{mock::*, Error};
use frame_support::{assert_noop, assert_ok};

#[test]
fn test_set_default_value_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(LeaseCommittee::set_min_stake(
            RawOrigin::Root.into(),
            10u32.into()
        ));

        assert_ok!(LeaseCommittee::set_alternate_committee_limit(
            RawOrigin::Root.into(),
            10u32
        ));

        assert_ok!(LeaseCommittee::set_committee_limit(
            RawOrigin::Root.into(),
            5u32
        ));

        // // Dispatch a signed extrinsic.
        // assert_ok!(TemplateModule::do_something(Origin::signed(1), 42));
        // // Read pallet storage and assert an expected result.
        // assert_eq!(TemplateModule::something(), Some(42));
    });
}
