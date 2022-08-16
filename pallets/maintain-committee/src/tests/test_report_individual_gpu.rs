use super::super::mock::*;
use frame_support::assert_ok;
use once_cell::sync::Lazy;

const controller: Lazy<sp_core::sr25519::Public> = Lazy::new(|| sr25519::Public::from(Sr25519Keyring::Eve));

const committee1: Lazy<sp_core::sr25519::Public> = Lazy::new(|| sr25519::Public::from(Sr25519Keyring::One));
const committee2: Lazy<sp_core::sr25519::Public> = Lazy::new(|| sr25519::Public::from(Sr25519Keyring::Two));
const committee3: Lazy<sp_core::sr25519::Public> = Lazy::new(|| sr25519::Public::from(Sr25519Keyring::Ferdie));

const reporter: Lazy<sp_core::sr25519::Public> = committee2;

#[test]
fn report_indivudual_gpu() {
    // 一个机器被两个人进行租用，其中一个进行举报，举报成功，将另一个进行下架。
    new_test_with_init_machine_online().execute_with(|| {
        let renter1 = sr25519::Public::from(Sr25519Keyring::Alice);
        let renter2 = sr25519::Public::from(Sr25519Keyring::Bob);

        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();

        // 两人各租用1台机器
        assert_ok!(RentMachine::rent_machine(Origin::signed(renter1), machine_id.clone(), 2, 1));
        // 检查状态
        {
            // rent_machine:
            // Balance: 支付10 DBC; UserTotalStake; NextRentId; RentOrder; UserRented; PendingRentEnding;
            // PendingConfirming; MachineRentOrder;
            //
            // online_profile:
            // machine_info; live_machines; machien_rented_gpu;
        }
        // FIXME: 无法租用
        // assert_ok!(RentMachine::rent_machine(Origin::signed(renter2), machine_id, 2, 1));
        {}
    })
}
