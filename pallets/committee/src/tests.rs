use frame_support::{assert_err, assert_noop, assert_ok};

use super::Error;
use crate::{mock::*, CommitteeList};

#[test]
fn add_committee_works() {
    new_test_with_init_params_ext().execute_with(|| {
        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();
        let committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One).into();
        let committee3: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Two).into();
        let committee4: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Alice).into();

        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee1));
        assert_eq!(
            Committee::committee(),
            super::CommitteeList { waiting_box_pubkey: vec![committee1], ..Default::default() },
        );

        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee2));
        assert_eq!(
            Committee::committee(),
            super::CommitteeList { waiting_box_pubkey: vec![committee1, committee2], ..Default::default() },
        );

        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee3));
        assert_eq!(
            Committee::committee(),
            super::CommitteeList { waiting_box_pubkey: vec![committee3, committee1, committee2], ..Default::default() },
        );

        // Add twice will faile
        assert_noop!(
            Committee::add_committee(RawOrigin::Root.into(), committee2),
            Error::<TestRuntime>::AccountAlreadyExist
        );

        // is_in_committee works:
        let tmp_committee_list = CommitteeList {
            normal: vec![committee1],
            chill_list: vec![committee2],
            waiting_box_pubkey: vec![committee3],
            fulfilling_list: vec![committee4],
        };
        assert_eq!(tmp_committee_list.is_in_committee(&committee3), true);
    })
}
