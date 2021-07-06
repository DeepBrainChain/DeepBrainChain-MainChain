use crate as lease_committee;
use frame_support::{
    parameter_types,
    traits::{OnFinalize, OnInitialize},
};
pub use frame_system::{self as system, RawOrigin};
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
    ModuleId, Permill,
};

use frame_system::EnsureRoot;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;
type Block = frame_system::mocking::MockBlock<TestRuntime>;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const SS58Prefix: u8 = 42;
}

impl system::Config for TestRuntime {
    type BaseCallFilter = ();
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type Origin = Origin;
    type Call = Call;
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = sr25519::Public;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = Event;
    type BlockHashCount = BlockHashCount;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<u128>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = SS58Prefix;
}

parameter_types! {
    pub const ExistentialDeposit: u64 = 1;
}

impl pallet_balances::Config for TestRuntime {
    type Balance = u128;
    type MaxLocks = ();
    type Event = Event;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
}

impl dbc_price_ocw::Config for TestRuntime {
    type Currency = Balances;
    type Event = Event;
    type RandomnessSource = RandomnessCollectiveFlip;
}

type TestExtrinsic = TestXt<Call, ()>;
impl<LocalCall> system::offchain::CreateSignedTransaction<LocalCall> for TestRuntime
where
    Call: From<LocalCall>,
{
    fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
        call: Call,
        _public: <Signature as Verify>::Signer,
        _account: <TestRuntime as system::Config>::AccountId,
        index: <TestRuntime as system::Config>::Index,
    ) -> Option<(Call, <TestExtrinsic as sp_runtime::traits::Extrinsic>::SignaturePayload)> {
        Some((call, (index, ())))
    }
}

impl frame_system::offchain::SigningTypes for TestRuntime {
    type Public = <Signature as Verify>::Signer;
    type Signature = Signature;
}

impl<C> frame_system::offchain::SendTransactionTypes<C> for TestRuntime
where
    Call: From<C>,
{
    type OverarchingCall = Call;
    type Extrinsic = TestExtrinsic;
}

parameter_types! {
    pub const BlockPerEra: u32 = 3600 * 24 / 30;
}

impl generic_func::Config for TestRuntime {
    type BlockPerEra = BlockPerEra;
    type Currency = Balances;
    type Event = Event;
    type RandomnessSource = RandomnessCollectiveFlip;
    type FixedTxFee = Treasury;
}

parameter_types! {
    pub const ProposalBond: Permill = Permill::from_percent(5);
    pub const ProposalBondMinimum: u64 = 1;
    pub const SpendPeriod: u64 = 2;
    pub const Burn: Permill = Permill::from_percent(50);
    pub const DataDepositPerByte: u64 = 1;
    pub const TreasuryModuleId: ModuleId = ModuleId(*b"py/trsry");
    pub const MaxApprovals: u32 = 100;
}

impl pallet_treasury::Config for TestRuntime {
    type ModuleId = TreasuryModuleId;
    type Currency = Balances;
    type ApproveOrigin = EnsureRoot<Self::AccountId>;
    type RejectOrigin = EnsureRoot<Self::AccountId>;
    type Event = Event;
    type OnSlash = ();
    type ProposalBond = ProposalBond;
    type ProposalBondMinimum = ProposalBondMinimum;
    type SpendPeriod = SpendPeriod;
    type Burn = Burn;
    type BurnDestination = (); // Just gets burned.
    type WeightInfo = ();
    type SpendFunds = ();
}

parameter_types! {
    pub const MinimumPeriod: u64 = 5;
}

impl pallet_timestamp::Config for TestRuntime {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

impl committee::Config for TestRuntime {
    type Currency = Balances;
    type Event = Event;
    type Slash = Treasury;
    type DbcPrice = DBCPriceOCW;
}

parameter_types! {
    pub const CommitteeDuration: u32 = 7;
}

impl lease_committee::Config for TestRuntime {
    type Event = Event;
    type Currency = Balances;
    type LCOperations = OnlineProfile;
    type ManageCommittee = Committee;
}

parameter_types! {
    pub const BondingDuration: u32 = 7;
    pub const ProfitReleaseDuration: u64 = 150;
}

impl online_profile::Config for TestRuntime {
    type Currency = Balances;
    type Event = Event;
    type BondingDuration = BondingDuration;
    // type ProfitReleaseDuration = ProfitReleaseDuration;
    type DbcPrice = DBCPriceOCW;
    type ManageCommittee = Committee;
}

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum TestRuntime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Module, Call, Config, Storage, Event<T>},
        LeaseCommittee: lease_committee::{Module, Call, Storage, Event<T>},
        OnlineProfile: online_profile::{Module, Call, Storage, Event<T>},
        RandomnessCollectiveFlip: pallet_randomness_collective_flip::{Module, Call, Storage},
        Balances: pallet_balances::{Module, Call, Storage, Event<T>},
        Committee: committee::{Module, Call, Storage, Event<T>},
        DBCPriceOCW: dbc_price_ocw::{Module, Call, Storage, Event<T>, ValidateUnsigned},
        Treasury: pallet_treasury::{Module, Call, Storage, Config, Event<T>},
        GenericFunc: generic_func::{Module, Call, Storage, Event<T>},
        Timestamp: pallet_timestamp::{Module, Call, Storage, Inherent},
    }
);

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = system::GenesisConfig::default().build_storage::<TestRuntime>().unwrap();
    let init_balance = 10000000000000000000000;

    #[rustfmt::skip]
    pallet_balances::GenesisConfig::<TestRuntime> {
        balances: vec![
            (sr25519::Public::from(Sr25519Keyring::Alice).into(), init_balance),
            (sr25519::Public::from(Sr25519Keyring::Bob).into(), init_balance),
            (sr25519::Public::from(Sr25519Keyring::Charlie).into(), init_balance),
            (sr25519::Public::from(Sr25519Keyring::Dave).into(), init_balance),
            (sr25519::Public::from(Sr25519Keyring::Eve).into(), init_balance),
            (sr25519::Public::from(Sr25519Keyring::Ferdie).into(), init_balance),
            (sr25519::Public::from(Sr25519Keyring::One).into(), init_balance),
            (sr25519::Public::from(Sr25519Keyring::Two).into(), init_balance),
        ],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    t.into()
}

pub type BlockNumber = u64;

pub fn run_to_block(n: BlockNumber) {
    for b in System::block_number()..=n {
        // 当前块结束
        OnlineProfile::on_finalize(b);
        LeaseCommittee::on_finalize(b);
        Committee::on_finalize(b);
        System::on_finalize(b);

        System::set_block_number(b + 1);

        // 下一块初始化
        System::on_initialize(b + 1);
        LeaseCommittee::on_initialize(b + 1);
        Committee::on_initialize(b + 1);
        OnlineProfile::on_initialize(b + 1);
    }
}
