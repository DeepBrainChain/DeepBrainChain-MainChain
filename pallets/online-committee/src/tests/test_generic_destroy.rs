use super::super::mock::*;
use frame_support::assert_ok;

// 测试用户主动销毁
#[test]
fn test_user_destroy() {
    new_test_with_init_params_ext().execute_with(|| {
        let alice = sr25519::Public::from(Sr25519Keyring::Alice);
        assert_ok!(GenericFunc::destroy_free_dbc(RuntimeOrigin::signed(alice), 1000));

        assert_eq!(Balances::free_balance(&alice), INIT_BALANCE - 1000);
        assert_eq!(Balances::total_issuance(), INIT_BALANCE * 8 - 1000);
    })
}

// 测试自动销毁金额
#[test]
fn test_auto_destroy() {
    new_test_with_init_params_ext().execute_with(|| {
        let alice = sr25519::Public::from(Sr25519Keyring::Alice);
        let bob = sr25519::Public::from(Sr25519Keyring::Bob);
        assert_ok!(GenericFunc::set_auto_destroy(RawOrigin::Root.into(), alice, 10));

        assert_eq!(Balances::free_balance(&alice), INIT_BALANCE);
        assert_eq!(Balances::total_issuance(), INIT_BALANCE * 8);

        run_to_block(10);
        assert_eq!(Balances::free_balance(&alice), 0);
        assert_eq!(Balances::total_issuance(), INIT_BALANCE * (8 - 1));

        assert_ok!(Balances::transfer(RuntimeOrigin::signed(bob), alice, 1000));

        assert_eq!(Balances::free_balance(&alice), 1000);
        run_to_block(20);
        assert_eq!(Balances::free_balance(&alice), 0);
        assert_eq!(Balances::total_issuance(), INIT_BALANCE * (8 - 1) - 1000);
    })
}
