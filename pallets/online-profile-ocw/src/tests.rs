use crate::mock::*;
use frame_support::assert_ok;
// use parking_lot::RwLock;
use sp_core::offchain::testing;
use sp_io::TestExternalities;
use sp_runtime::offchain::{OffchainExt, TransactionPoolExt};

#[test]
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
fn ocw_fetch_machine_info_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let alice: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Alice).into();
        let bob: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Bob).into();
        let charile: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Charlie).into();
        let dave: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Dave).into();
        let eve: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();

        // 初始化单显卡最小质押
        assert_ok!(OnlineProfile::set_gpu_stake(RawOrigin::Root.into(), 200_000u32.into()));
        assert_ok!(OnlineProfileOcw::add_machine_info_url(RawOrigin::Root.into(), "http://127.0.0.1:8000".into()));
        assert_ok!(OnlineProfileOcw::add_machine_info_url(RawOrigin::Root.into(), "http://127.0.0.1:8001".into()));
        assert_ok!(OnlineProfileOcw::add_machine_info_url(RawOrigin::Root.into(), "http://127.0.0.1:8002".into()));
        assert_ok!(OnlineProfileOcw::add_machine_info_url(RawOrigin::Root.into(), "http://127.0.0.1:8003".into()));

        assert_eq!(OnlineProfileOcw::machine_info_url().len(), 4);

        // 增加URL将会自动更新url group
        System::set_block_number(2);
        assert_eq!(OnlineProfileOcw::machine_info_rand_url().len(), 3);

        // dave用户绑定机器：
        let machine_id = "abcdefg";
        assert_ok!(OnlineProfile::bond_machine(Origin::signed(dave), machine_id.into(), 3));

        System::set_block_number(3);
    })
}

struct ExternalityBuilder;

// impl ExternalityBuilder {
//     pub fn build() -> (TestExternalities, Arc<RwLock<PoolState>>, Arc<RwLock<OffchainState>>) {
//         const PHRASE: &str = "expire stage crawl shell boss any story swamp skull yello bamboo copy";

//         let (offchain, offchain_state) = testing::TestOffchainExt::new();
//         let (pool, pool_state) = testing::TestTransactionPoolExt::new();
//         let keystore = KeyStore::new();
//         keystore.write().sr25519_generate_new(KEY_TYPE, Some(&format!("{}/hunter1", PHRASE))).unwrap();
//         let storage = system::GenesisConfig::default().build_storage::<TestRuntime>().unwrap();

//         let mut t = TestExternalities::from(storage);
//         t.register_extension(OffchainExt::new(offchain));
//         t.register_extension(TransactionPoolExt::new(pool));
//         t.register_extension(KeystoreExt(keystore));
//         t.execute_with(|| System::set_block_number(1));
//         (t, pool_state, offchain_state)
//     }
// }

#[test]
fn should_make_http_call_and_parse_result() {
    let (offchain, state) = testing::TestOffchainExt::new();
    let mut t = sp_io::TestExternalities::default();
    t.register_extension(OffchainExt::new(offchain));

    // price_oracle_response(&mut state.write());

    t.execute_with(|| {
        // when
        // let price = OnlineProfileOcw::fetch_price().unwrap();
        // then
        // assert_eq!(price, 15523);
    });
}

// 模拟一个http的输出
// fn price_oracle_response(state: &mut testing::OffchainState) {
//     state.expect_request(testing::PendingRequest {
//         method: "GET".into(),
//         uri: "https://min-api.cryptocompare.com/data/price?fsym=BTC&tsyms=USD".into(),
//         response: Some(br#"{"USD": 155.23}"#.to_vec()),
//         sent: true,
//         ..Default::default()
//     });
// }
