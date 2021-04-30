use crate::mock::*;
use frame_support::assert_ok;
// use codec::Encode;

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

        // 设置单个GPU质押数量
        assert_ok!(OnlineProfile::set_gpu_stake(RawOrigin::Root.into(), 200_000u32.into()));
        assert_eq!(OnlineProfile::stake_per_gpu(), 200_000);

        let machine_id = "abcdefg";
        assert_ok!(OnlineProfile::bond_machine(Origin::signed(dave), machine_id.into(), 3));

        let user_machines = OnlineProfile::user_machines(dave);
        assert_eq!(user_machines.len(), 1);

        let live_machines = OnlineProfile::live_machines();
        assert_eq!(live_machines.bonding_machine.len(), 1);
        assert_eq!(live_machines.ocw_confirmed_machine.len(), 0);

        let _machine_info = OnlineProfile::machines_info(machine_id.as_bytes());
        let _ledger = OnlineProfile::ledger(dave, machine_id.as_bytes());

        // 检查已锁定的金额
        let locked_balance = Balances::locks(dave);
        assert_eq!(locked_balance.len(), 1);
        assert_eq!(locked_balance[0].id, "oprofile".as_bytes());
        assert_eq!(locked_balance[0].amount, 600_000);
    });
}

#[test]
fn unbond_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
    });
}
