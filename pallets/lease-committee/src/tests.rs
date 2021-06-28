use crate::mock::*;
use frame_support::assert_ok;

#[test]
fn set_default_value_works() {
    // 测试初始化设置参数
    new_test_ext().execute_with(|| {
        assert_ok!(Committee::set_staked_usd_per_order(RawOrigin::Root.into(), 500_0000u32.into()));
        assert_ok!(GenericFunc::set_fixed_tx_fee(RawOrigin::Root.into(), 5u32.into()));
        assert_ok!(OnlineProfile::set_gpu_stake(RawOrigin::Root.into(), 3u32.into()));
    });
}

#[test]
fn select_committee_works() {
    // 质押--参加选举--当选
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let alice: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Alice).into();
        let bob: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Bob).into();
        let charile: sp_core::sr25519::Public =
            sr25519::Public::from(Sr25519Keyring::Charlie).into();
        let dave: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Dave).into();
        let eve: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();

        assert_eq!(Balances::free_balance(alice), 1000_000);

        // 设置初始值
        let _ = LeaseCommittee::set_min_stake(RawOrigin::Root.into(), 500_000u32.into());
        let _ = LeaseCommittee::set_alternate_committee_limit(RawOrigin::Root.into(), 5u32);
        let _ = LeaseCommittee::set_committee_limit(RawOrigin::Root.into(), 3u32);

        // 参加选举，成为候选人
        assert_ok!(LeaseCommittee::stake_for_alternate_committee(
            Origin::signed(alice),
            500_000u32.into()
        ));
        assert_ok!(LeaseCommittee::stake_for_alternate_committee(
            Origin::signed(bob),
            500_000u32.into()
        ));
        assert_ok!(LeaseCommittee::stake_for_alternate_committee(
            Origin::signed(charile),
            500_000u32.into()
        ));
        assert_ok!(LeaseCommittee::stake_for_alternate_committee(
            Origin::signed(dave),
            500_000u32.into()
        ));
        assert_ok!(LeaseCommittee::stake_for_alternate_committee(
            Origin::signed(eve),
            500_000u32.into()
        ));

        assert_eq!(LeaseCommittee::alternate_committee().len(), 5);
        assert_ok!(LeaseCommittee::reelection_committee(RawOrigin::Root.into()));

        assert_eq!(LeaseCommittee::committee().len(), 3);
        assert_eq!(LeaseCommittee::alternate_committee().len(), 5);
    })
}

#[test]
fn book_one_machine_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
    })
}

#[test]
fn bool_all_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
    })
}

#[test]
fn white_list_works() {
    // white_list 总能被选为committee
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
    });
}

#[test]
fn black_list_works() {
    // black_list 被禁止参加选举
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
    });
}
