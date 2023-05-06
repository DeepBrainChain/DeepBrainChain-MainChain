use crate as terminating_rental;
use dbc_support::report::ReporterStakeParamsInfo;
use frame_support::{
    assert_ok, parameter_types,
    traits::{ConstU32, OnFinalize, OnInitialize},
    PalletId,
};
use frame_system::{EnsureRoot, EnsureWithSuccess, RawOrigin};
pub use sp_core::{
    sr25519::{self, Signature},
    H256,
};
pub use sp_keyring::{
    ed25519::Keyring as Ed25519Keyring, sr25519::Keyring as Sr25519Keyring, AccountKeyring,
};
use sp_runtime::{
    testing::{Header, TestXt},
    traits::{BlakeTwo256, IdentityLookup, Verify},
    Perbill, Permill,
};
use std::convert::TryInto;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;
type Block = frame_system::mocking::MockBlock<TestRuntime>;
type Balance = u128;
pub type BlockNumber = u64;

// 1 DBC = 1 * 10^15
pub const ONE_DBC: u128 = 1_000_000_000_000_000;
// 初始1000WDBC
pub const INIT_BALANCE: u128 = 10_000_000 * ONE_DBC;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const SS58Prefix: u8 = 42;
}

impl frame_system::Config for TestRuntime {
    type BaseCallFilter = ();
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = sr25519::Public;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = BlockHashCount;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<u128>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = SS58Prefix;
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
}

parameter_types! {
    pub const ExistentialDeposit: u64 = 1;
    pub const MaxReservers: u32 = 50;
}

impl pallet_balances::Config for TestRuntime {
    type MaxLocks = ();
    type MaxReserves = MaxReservers;
    type ReserveIdentifier = [u8; 8];
    type Balance = u128;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
}

parameter_types! {
    pub const BlockPerEra: u32 = 3600 * 24 / 30;
}

impl pallet_randomness_collective_flip::Config for TestRuntime {}

impl generic_func::Config for TestRuntime {
    type BlockPerEra = BlockPerEra;
    type Currency = Balances;
    type RuntimeEvent = RuntimeEvent;
    type RandomnessSource = RandomnessCollectiveFlip;
    type FixedTxFee = Treasury;
    type Slash = Treasury;
}

parameter_types! {
    pub const ProposalBond: Permill = Permill::from_percent(5);
    pub const ProposalBondMinimum: u64 = 1;
    pub const SpendPeriod: u64 = 2;
    pub const Burn: Permill = Permill::from_percent(50);
    pub const DataDepositPerByte: u64 = 1;
    pub const TreasuryModuleId: PalletId = PalletId(*b"py/trsry");
    pub const MaxApprovals: u32 = 100;
    pub const MaxBalance: Balance = Balance::max_value();
}

impl pallet_treasury::Config for TestRuntime {
    type PalletId = TreasuryModuleId;
    type Currency = Balances;
    type ApproveOrigin = EnsureRoot<Self::AccountId>;
    type RejectOrigin = EnsureRoot<Self::AccountId>;
    type RuntimeEvent = RuntimeEvent;
    type OnSlash = ();
    type ProposalBond = ProposalBond;
    type ProposalBondMinimum = ProposalBondMinimum;
    type SpendPeriod = SpendPeriod;
    type Burn = Burn;
    type BurnDestination = (); // Just gets burned.
    type WeightInfo = ();
    type SpendFunds = ();

    type ProposalBondMaximum = ();
    type MaxApprovals = MaxApprovals;
    type SpendOrigin = EnsureWithSuccess<EnsureRoot<Self::AccountId>, Self::AccountId, MaxBalance>;
}

impl committee::Config for TestRuntime {
    type Currency = Balances;
    type RuntimeEvent = RuntimeEvent;
    // type WeightInfo = ();
}

impl terminating_rental::Config for TestRuntime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type Slash = Treasury;
    type ManageCommittee = Committee;
    type DbcPrice = DBCPriceOCW;
    type SlashAndReward = GenericFunc;
}

impl dbc_price_ocw::Config for TestRuntime {
    type Currency = Balances;
    type RuntimeEvent = RuntimeEvent;
    type RandomnessSource = RandomnessCollectiveFlip;
}

type TestExtrinsic = TestXt<RuntimeCall, ()>;
impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for TestRuntime
where
    RuntimeCall: From<LocalCall>,
{
    fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
        call: RuntimeCall,
        _public: <Signature as Verify>::Signer,
        _account: <TestRuntime as frame_system::Config>::AccountId,
        index: <TestRuntime as frame_system::Config>::Index,
    ) -> Option<(RuntimeCall, <TestExtrinsic as sp_runtime::traits::Extrinsic>::SignaturePayload)>
    {
        Some((call, (index, ())))
    }
}

impl frame_system::offchain::SigningTypes for TestRuntime {
    type Public = <Signature as Verify>::Signer;
    type Signature = Signature;
}

impl<C> frame_system::offchain::SendTransactionTypes<C> for TestRuntime
where
    RuntimeCall: From<C>,
{
    type OverarchingCall = RuntimeCall;
    type Extrinsic = TestExtrinsic;
}

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum TestRuntime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system,
        // Timestamp: pallet_timestamp::{Module, Call, Storage, Inherent},
        // OnlineCommittee: online_committee::{Module, Call, Storage, Event<T>},
        // OnlineProfile: online_profile::{Module, Call, Storage, Event<T>},
        RandomnessCollectiveFlip: pallet_randomness_collective_flip,
        Balances: pallet_balances,
        Committee: committee,
        DBCPriceOCW: dbc_price_ocw,
        Treasury: pallet_treasury,
        GenericFunc: generic_func,
        // RentMachine: rent_machine::{Module, Storage, Call, Event<T>},
        // TechnicalCommittee: pallet_collective::<Instance2>::{Module, Call, Storage, Origin<T>, Event<T>, Config<T>},
        TerminatingRental: terminating_rental,
    }
);

pub fn run_to_block(n: BlockNumber) {
    for b in System::block_number()..=n {
        // 当前块结束
        // OnlineCommittee::on_finalize(b);
        // OnlineProfile::on_finalize(b);
        // Committee::on_finalize(b);
        System::on_finalize(b);
        RandomnessCollectiveFlip::on_finalize(b);
        // Timestamp::set_timestamp(System::block_number() * BLOCK_TIME + INIT_TIMESTAMP);
        TerminatingRental::on_finalize(b);

        System::set_block_number(b + 1);

        // 下一块初始化
        RandomnessCollectiveFlip::on_initialize(b + 1);
        System::on_initialize(b + 1);
        // OnlineCommittee::on_initialize(b + 1);
        // Committee::on_initialize(b + 1);
        // OnlineProfile::on_initialize(b + 1);
        RandomnessCollectiveFlip::on_initialize(b + 1);
        GenericFunc::on_initialize(b + 1);
        TerminatingRental::on_initialize(b + 1);
    }
}

// 初始条件：只设置初始参数
// Build genesis storage according to the mock runtime.
pub fn new_test_with_init_params_ext() -> sp_io::TestExternalities {
    let mut storage =
        frame_system::GenesisConfig::default().build_storage::<TestRuntime>().unwrap();

    // 初始化测试帐号余额
    #[rustfmt::skip]
    pallet_balances::GenesisConfig::<TestRuntime> {
        balances: vec![
            (sr25519::Public::from(Sr25519Keyring::Alice).into(), INIT_BALANCE),
            (sr25519::Public::from(Sr25519Keyring::Bob).into(), INIT_BALANCE),
            (sr25519::Public::from(Sr25519Keyring::Charlie).into(), INIT_BALANCE),
            (sr25519::Public::from(Sr25519Keyring::Dave).into(), INIT_BALANCE),
            (sr25519::Public::from(Sr25519Keyring::Eve).into(), INIT_BALANCE),
            (sr25519::Public::from(Sr25519Keyring::Ferdie).into(), INIT_BALANCE),
            (sr25519::Public::from(Sr25519Keyring::One).into(), INIT_BALANCE),
            (sr25519::Public::from(Sr25519Keyring::Two).into(), INIT_BALANCE),
        ],
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    let mut ext = sp_io::TestExternalities::from(storage);

    ext.execute_with(|| {
        run_to_block(1);

        for _ in 0..64 {
            DBCPriceOCW::add_price(12_000u64);
        }
        DBCPriceOCW::add_avg_price();
        assert_eq!(DBCPriceOCW::avg_price(), Some(12_000u64));

        // 设置标准GPU租金价格: (3080得分1000；租金每月1000RMB) {1000; 150_000_000};
        assert_ok!(TerminatingRental::set_standard_gpu_point_price(
            RawOrigin::Root.into(),
            dbc_support::machine_type::StandardGpuPointPrice {
                gpu_point: 1000,
                gpu_price: 5_000_000
            }
        ));

        assert_ok!(TerminatingRental::set_stake_per_gpu(RawOrigin::Root.into(), 10000 * ONE_DBC));

        // 初始化设置参数
        // 委员会每次抢单质押数量 (1000 DBC)
        assert_ok!(Committee::set_committee_stake_params(
            RawOrigin::Root.into(),
            committee::CommitteeStakeParamsInfo {
                stake_baseline: 20000 * ONE_DBC,
                stake_per_order: 1000 * ONE_DBC,
                min_free_stake_percent: Perbill::from_rational_approximation(40u32, 100u32),
            },
        ));

        assert_ok!(TerminatingRental::set_reporter_stake_params(
            RawOrigin::Root.into(),
            ReporterStakeParamsInfo {
                stake_baseline: 20000 * ONE_DBC,
                stake_per_report: 1000 * ONE_DBC,
                min_free_stake_percent: Perbill::from_percent(40)
            }
        ));

        // 操作时的固定费率: 10 DBC
        assert_ok!(GenericFunc::set_fixed_tx_fee(RawOrigin::Root.into(), 10 * ONE_DBC));
        // 上线时的保证金
        assert_ok!(TerminatingRental::set_online_deposit(RawOrigin::Root.into(), 10000 * ONE_DBC));

        let committee1 = sr25519::Public::from(Sr25519Keyring::Alice);
        let committee2 = sr25519::Public::from(Sr25519Keyring::Charlie);
        let committee3 = sr25519::Public::from(Sr25519Keyring::Dave);
        let committee4 = sr25519::Public::from(Sr25519Keyring::Eve);

        // 增加四个委员会
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee1));
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee2));
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee3));
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee4));
        let committee1_box_pubkey =
            hex::decode("ff3033c763f71bc51f372c1dc5095accc26880e138df84cac13c46bfd7dbd74f")
                .unwrap()
                .try_into()
                .unwrap();
        let committee2_box_pubkey =
            hex::decode("336404f7d316565cc3c3350e70561f4177803e0bb02a7f2e4e02a4f0e361157e")
                .unwrap()
                .try_into()
                .unwrap();
        let committee3_box_pubkey =
            hex::decode("a7804e30caa5645e97489b2d4711e3d8f4e17a683338cba97a53b960648f0438")
                .unwrap()
                .try_into()
                .unwrap();
        let committee4_box_pubkey =
            hex::decode("5eec53877f4b18c8b003fa983d27ef2e5518b7e4d08d482922a7787f2ea75529")
                .unwrap()
                .try_into()
                .unwrap();

        assert_ok!(Committee::committee_set_box_pubkey(
            RuntimeOrigin::signed(committee1),
            committee1_box_pubkey
        ));
        assert_ok!(Committee::committee_set_box_pubkey(
            RuntimeOrigin::signed(committee2),
            committee2_box_pubkey
        ));
        assert_ok!(Committee::committee_set_box_pubkey(
            RuntimeOrigin::signed(committee3),
            committee3_box_pubkey
        ));
        assert_ok!(Committee::committee_set_box_pubkey(
            RuntimeOrigin::signed(committee4),
            committee4_box_pubkey
        ));
    });

    ext
}
