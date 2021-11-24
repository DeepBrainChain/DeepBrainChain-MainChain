use super::Error;
use crate::{mock::*, CommitteeList};
use frame_support::{assert_err, assert_noop, assert_ok};
use std::{collections::BTreeMap, convert::TryInto};

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

#[test]
fn committee_set_box_pubkey_works() {
    new_test_with_init_params_ext().execute_with(|| {
        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();
        let committee1_box_pubkey: [u8; 32] =
            hex::decode("ff3033c763f71bc51f372c1dc5095accc26880e138df84cac13c46bfd7dbd74f")
                .unwrap()
                .try_into()
                .unwrap();
        let stake_params = Committee::committee_stake_params().unwrap();

        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee1));
        assert_ok!(Committee::committee_set_box_pubkey(Origin::signed(committee1), committee1_box_pubkey.clone()));

        assert_eq!(Committee::committee(), super::CommitteeList { normal: vec![committee1], ..Default::default() });
        assert_eq!(
            Committee::committee_stake(&committee1),
            super::CommitteeStakeInfo {
                box_pubkey: committee1_box_pubkey,
                staked_amount: stake_params.stake_baseline,
                used_stake: 0,
                can_claim_reward: 0,
                claimed_reward: 0,
            }
        );
        assert_eq!(Balances::reserved_balance(&committee1), 20000 * ONE_DBC);

        // if committee is in normal list, can change directly
        let committee1_box_pubkey2: [u8; 32] =
            hex::decode("9dccbab2d61405084eac440f877a6479bc827373b2e414e81a6170ebe5aadd12")
                .unwrap()
                .try_into()
                .unwrap();

        assert_ok!(Committee::committee_set_box_pubkey(Origin::signed(committee1), committee1_box_pubkey2));
    })
}

#[test]
fn committee_add_stake_works() {
    new_test_with_init_params_ext().execute_with(|| {
        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();
        let committee1_box_pubkey: [u8; 32] =
            hex::decode("ff3033c763f71bc51f372c1dc5095accc26880e138df84cac13c46bfd7dbd74f")
                .unwrap()
                .try_into()
                .unwrap();
        let stake_params = Committee::committee_stake_params().unwrap();

        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee1));
        assert_ok!(Committee::committee_set_box_pubkey(Origin::signed(committee1), committee1_box_pubkey.clone()));

        assert_ok!(Committee::committee_add_stake(Origin::signed(committee1), 5000 * ONE_DBC));

        assert_eq!(
            Committee::committee_stake(&committee1),
            super::CommitteeStakeInfo {
                box_pubkey: committee1_box_pubkey,
                staked_amount: stake_params.stake_baseline + 5000 * ONE_DBC,
                used_stake: 0,
                can_claim_reward: 0,
                claimed_reward: 0,
            }
        );
        assert_eq!(Balances::reserved_balance(&committee1), 25000 * ONE_DBC);
    })
}

#[test]
fn committee_reduce_stake_works() {
    new_test_with_init_params_ext().execute_with(|| {
        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();
        let committee1_box_pubkey: [u8; 32] =
            hex::decode("ff3033c763f71bc51f372c1dc5095accc26880e138df84cac13c46bfd7dbd74f")
                .unwrap()
                .try_into()
                .unwrap();
        let stake_params = Committee::committee_stake_params().unwrap();

        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee1));
        assert_ok!(Committee::committee_set_box_pubkey(Origin::signed(committee1), committee1_box_pubkey.clone()));

        assert_ok!(Committee::committee_add_stake(Origin::signed(committee1), 5000 * ONE_DBC));

        assert_ok!(Committee::committee_reduce_stake(Origin::signed(committee1), 4000 * ONE_DBC));

        assert_eq!(
            Committee::committee_stake(&committee1),
            super::CommitteeStakeInfo {
                box_pubkey: committee1_box_pubkey,
                staked_amount: stake_params.stake_baseline + 1000 * ONE_DBC,
                used_stake: 0,
                can_claim_reward: 0,
                claimed_reward: 0,
            }
        );
        assert_eq!(Balances::reserved_balance(&committee1), 21000 * ONE_DBC);

        assert_noop!(
            Committee::committee_reduce_stake(Origin::signed(committee1), 2000 * ONE_DBC),
            Error::<TestRuntime>::BalanceNotEnough
        );
    })
}

#[test]
fn committee_claim_reward_works() {
    new_test_with_init_params_ext().execute_with(|| {
        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();

        super::CommitteeStake::<TestRuntime>::insert(
            &committee1,
            super::CommitteeStakeInfo { can_claim_reward: 1000 * ONE_DBC, ..Default::default() },
        );

        assert_ok!(Committee::claim_reward(Origin::signed(committee1)));
        assert_eq!(Balances::free_balance(&committee1), INIT_BALANCE + 1000 * ONE_DBC);
        assert_eq!(
            Committee::committee_stake(&committee1),
            super::CommitteeStakeInfo { claimed_reward: 1000 * ONE_DBC, ..Default::default() }
        );
    })
}

#[test]
fn committee_chill_works() {
    new_test_with_init_params_ext().execute_with(|| {
        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();

        super::Committee::<TestRuntime>::put(super::CommitteeList { normal: vec![committee1], ..Default::default() });
        assert_ok!(Committee::chill(Origin::signed(committee1)));

        assert_eq!(Committee::committee(), super::CommitteeList { chill_list: vec![committee1], ..Default::default() });

        // TODO: check to ensure if committee is in chill list, will not be changed to other list
        // `change_committee_status_when_stake_changed`
    })
}

#[test]
fn committee_undo_chill_works() {
    new_test_with_init_params_ext().execute_with(|| {})
}

#[test]
fn committee_exit_works() {
    new_test_with_init_params_ext().execute_with(|| {})
}
