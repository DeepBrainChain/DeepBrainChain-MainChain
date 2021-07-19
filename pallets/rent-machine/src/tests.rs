use crate::mock::*;
use frame_support::assert_ok;

#[test]
fn rent_machine_should_works() {
    new_test_with_online_machine_online_ext().execute_with(|| {
        let _alice: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Alice).into();
        let _bob: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Bob).into();
        let _charile: sp_core::sr25519::Public =
            sr25519::Public::from(Sr25519Keyring::Charlie).into();
        let renter_dave: sp_core::sr25519::Public =
            sr25519::Public::from(Sr25519Keyring::Dave).into();

        let _one_committee: sp_core::sr25519::Public =
            sr25519::Public::from(Sr25519Keyring::One).into();
        let _pot_two: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Two).into();

        let _controller: sp_core::sr25519::Public =
            sr25519::Public::from(Sr25519Keyring::Eve).into();
        let _stash: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();
        let machine_id =
            "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();

        // dave租用了10天
        assert_ok!(RentMachine::rent_machine(Origin::signed(renter_dave), machine_id.clone(), 10));

        run_to_block(50);

        // dave确认租用成功
        assert_ok!(RentMachine::confirm_rent(Origin::signed(renter_dave), machine_id.clone()));

        // dave续租成功
        assert_ok!(RentMachine::relet_machine(Origin::signed(renter_dave), machine_id.clone(), 10));

        // TODO: 检查机器得分

        // TODO: 检查租金是否正确扣除

        // TODO: 检查机器退租后，状态是否清理

        // TODO: 检查机器没有租用成功，押金正常退回
    })
}

#[test]
fn controller_report_offline_when_online_should_work() {
    new_test_with_online_machine_online_ext().execute_with(|| {
        let controller: sp_core::sr25519::Public =
            sr25519::Public::from(Sr25519Keyring::Eve).into();
        let stash: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();
        let machine_id =
            "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();

        run_to_block(50);

        assert_ok!(OnlineProfile::controller_report_offline(
            Origin::signed(controller),
            machine_id.clone()
        ));
    })
}

// Case1: after report online, machine status is still rented
#[test]
fn controller_report_offline_when_rented_should_work1() {
    new_test_with_online_machine_online_ext().execute_with(|| {
        let controller: sp_core::sr25519::Public =
            sr25519::Public::from(Sr25519Keyring::Eve).into();
        let stash: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();
        let machine_id =
            "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();

        run_to_block(50);

        // 机器报告下线，查询存储

        // 机器报告上线，查询存储
    })
}

// Case2: after report online, machine is out of rent duration
#[test]
fn controller_report_offline_when_rented_should_work2() {
    new_test_with_online_machine_online_ext().execute_with(|| {
        let controller: sp_core::sr25519::Public =
            sr25519::Public::from(Sr25519Keyring::Eve).into();
        let stash: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();
        let machine_id =
            "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();

        run_to_block(50);
    })
}
