use super::Error;
use crate::{mock::*, CommitteeList};
use frame_support::{assert_noop, assert_ok};
use std::convert::TryInto;

#[test]
fn add_committee_works() {
    new_test_with_init_params_ext().execute_with(|| {
        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie);
        let committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One);
        let committee3: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Two);
        let committee4: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Alice);

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
        assert!(tmp_committee_list.is_committee(&committee3));
    })
}

#[test]
fn committee_set_box_pubkey_works() {
    new_test_with_init_params_ext().execute_with(|| {
        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie);
        let committee1_box_pubkey: [u8; 32] =
            hex::decode("ff3033c763f71bc51f372c1dc5095accc26880e138df84cac13c46bfd7dbd74f")
                .unwrap()
                .try_into()
                .unwrap();
        let stake_params = Committee::committee_stake_params().unwrap();

        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee1));
        assert_ok!(Committee::committee_set_box_pubkey(Origin::signed(committee1), committee1_box_pubkey));

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
        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie);
        let committee1_box_pubkey: [u8; 32] =
            hex::decode("ff3033c763f71bc51f372c1dc5095accc26880e138df84cac13c46bfd7dbd74f")
                .unwrap()
                .try_into()
                .unwrap();
        let stake_params = Committee::committee_stake_params().unwrap();

        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee1));
        assert_ok!(Committee::committee_set_box_pubkey(Origin::signed(committee1), committee1_box_pubkey));

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
        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie);
        let committee1_box_pubkey: [u8; 32] =
            hex::decode("ff3033c763f71bc51f372c1dc5095accc26880e138df84cac13c46bfd7dbd74f")
                .unwrap()
                .try_into()
                .unwrap();
        let stake_params = Committee::committee_stake_params().unwrap();

        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee1));
        assert_ok!(Committee::committee_set_box_pubkey(Origin::signed(committee1), committee1_box_pubkey));

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
        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie);

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
        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie);
        let committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One);
        let committee3: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Two);
        let committee4: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Alice);

        super::Committee::<TestRuntime>::put(super::CommitteeList {
            normal: vec![committee1],
            chill_list: vec![committee2],
            waiting_box_pubkey: vec![committee3],
            fulfilling_list: vec![committee4],
        });
        assert_ok!(Committee::chill(Origin::signed(committee1)));
        assert_ok!(Committee::chill(Origin::signed(committee2)));
        assert_noop!(Committee::chill(Origin::signed(committee3)), Error::<TestRuntime>::PubkeyNotSet);
        assert_ok!(Committee::chill(Origin::signed(committee4)));

        assert_eq!(
            Committee::committee(),
            super::CommitteeList {
                chill_list: vec![committee1, committee2, committee4],
                waiting_box_pubkey: vec![committee3],
                ..Default::default()
            }
        );
    })
}

#[test]
fn committee_undo_chill_works() {
    new_test_with_init_params_ext().execute_with(|| {
        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie);
        let committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One);
        let committee3: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Two);
        let committee4: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Alice);

        // insert
        super::Committee::<TestRuntime>::put(super::CommitteeList {
            normal: vec![committee1],
            chill_list: vec![committee2],
            waiting_box_pubkey: vec![committee3],
            fulfilling_list: vec![committee4],
        });
        super::CommitteeStake::<TestRuntime>::insert(
            &committee2,
            super::CommitteeStakeInfo { staked_amount: 20000 * ONE_DBC, ..Default::default() },
        );

        assert_noop!(Committee::undo_chill(Origin::signed(committee1)), Error::<TestRuntime>::NotInChillList);
        assert_ok!(Committee::undo_chill(Origin::signed(committee2)));

        assert_eq!(
            Committee::committee(),
            super::CommitteeList {
                normal: vec![committee1, committee2],
                chill_list: vec![],
                waiting_box_pubkey: vec![committee3],
                fulfilling_list: vec![committee4],
            }
        );

        super::Committee::<TestRuntime>::put(super::CommitteeList {
            normal: vec![committee1],
            chill_list: vec![committee2],
            waiting_box_pubkey: vec![committee3],
            fulfilling_list: vec![committee4],
        });
        super::CommitteeStake::<TestRuntime>::insert(
            &committee2,
            super::CommitteeStakeInfo {
                staked_amount: 20000 * ONE_DBC,
                used_stake: 13000 * ONE_DBC,
                ..Default::default()
            },
        );
        assert_ok!(Committee::undo_chill(Origin::signed(committee2)));
        assert_eq!(
            Committee::committee(),
            super::CommitteeList {
                normal: vec![committee1],
                chill_list: vec![],
                waiting_box_pubkey: vec![committee3],
                fulfilling_list: vec![committee2, committee4],
            }
        );
    })
}

#[test]
fn committee_exit_works() {
    new_test_with_init_params_ext().execute_with(|| {
        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie);
        let committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One);

        // insert
        super::Committee::<TestRuntime>::put(super::CommitteeList {
            normal: vec![committee1],
            chill_list: vec![committee2],
            ..Default::default()
        });

        assert_ok!(Committee::exit_committee(Origin::signed(committee2)));

        assert_eq!(Committee::committee(), super::CommitteeList { normal: vec![committee1], ..Default::default() });
    })
}

// check to ensure if committee is in chill list, will not be changed to other list
// `change_committee_status_when_stake_changed`
#[test]
fn change_committee_status_when_stake_changed_works() {
    new_test_with_init_params_ext().execute_with(|| {
        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie);
        let committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One);
        let committee3: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Two);
        let committee4: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Alice);

        let mut committee_list = super::CommitteeList {
            normal: vec![committee1],
            chill_list: vec![committee2],
            waiting_box_pubkey: vec![committee3],
            fulfilling_list: vec![committee4],
        };

        // Normal -> fulfilling
        let committee1_stake = super::CommitteeStakeInfo {
            staked_amount: 20000 * ONE_DBC,
            used_stake: 13000 * ONE_DBC,
            ..Default::default()
        };
        Committee::do_change_status_when_stake_changed(committee1, &mut committee_list, &committee1_stake);
        assert_eq!(
            committee_list,
            super::CommitteeList {
                normal: vec![],
                chill_list: vec![committee2],
                waiting_box_pubkey: vec![committee3],
                fulfilling_list: vec![committee1, committee4]
            }
        );

        // In Chill will not change
        let committee2_stake = super::CommitteeStakeInfo {
            staked_amount: 20000 * ONE_DBC,
            used_stake: 13000 * ONE_DBC,
            ..Default::default()
        };
        Committee::do_change_status_when_stake_changed(committee2, &mut committee_list, &committee2_stake);
        assert_eq!(
            committee_list,
            super::CommitteeList {
                chill_list: vec![committee2],
                waiting_box_pubkey: vec![committee3],
                fulfilling_list: vec![committee1, committee4],
                ..Default::default()
            }
        );

        // WaitingPubkey will not change
        let committee3_stake = super::CommitteeStakeInfo {
            staked_amount: 20000 * ONE_DBC,
            used_stake: 12000 * ONE_DBC,
            ..Default::default()
        };
        Committee::do_change_status_when_stake_changed(committee3, &mut committee_list, &committee3_stake);
        assert_eq!(
            committee_list,
            super::CommitteeList {
                chill_list: vec![committee2],
                waiting_box_pubkey: vec![committee3],
                fulfilling_list: vec![committee1, committee4],
                ..Default::default()
            }
        );

        // FUlling -> Normal
        let committee4_stake = super::CommitteeStakeInfo {
            staked_amount: 20000 * ONE_DBC,
            used_stake: 12000 * ONE_DBC,
            ..Default::default()
        };
        Committee::do_change_status_when_stake_changed(committee4, &mut committee_list, &committee4_stake);
        assert_eq!(
            committee_list,
            super::CommitteeList {
                normal: vec![committee4],
                chill_list: vec![committee2],
                waiting_box_pubkey: vec![committee3],
                fulfilling_list: vec![committee1],
            }
        );
    })
}
