use crate as lease_committee;
use frame_support::parameter_types;
pub use frame_system::{self as system, RawOrigin};
pub use sp_core::{
    sr25519::{self, Signature},
    H256,
};
pub use sp_keyring::{
    ed25519::Keyring as Ed25519Keyring, sr25519::Keyring as Sr25519Keyring, AccountKeyring,
};
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
    Perbill,
};

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
    type AccountData = pallet_balances::AccountData<u64>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = SS58Prefix;
}

parameter_types! {
    pub const ExistentialDeposit: u64 = 1;
}

impl pallet_balances::Config for TestRuntime {
    type Balance = u64;
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

parameter_types! {
    pub const ProposalBond: Permill = Permill::from_percent(5);
    pub const ProposalBondMinimum: u64 = 1;
    pub const SpendPeriod: u64 = 2;
    pub const Burn: Permill = Permill::from_percent(50);
    pub const DataDepositPerByte: u64 = 1;
    pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
    pub const MaxApprovals: u32 = 100;
}
// impl pallet_treasury::Config for Test {
impl pallet_treasury::Config for TestRuntime {
    type PalletId = TreasuryPalletId;
    type Currency = pallet_balances::Pallet<Test>;
    type ApproveOrigin = frame_system::EnsureRoot<u128>;
    type RejectOrigin = frame_system::EnsureRoot<u128>;
    type Event = Event;
    type OnSlash = ();
    type ProposalBond = ProposalBond;
    type ProposalBondMinimum = ProposalBondMinimum;
    type SpendPeriod = SpendPeriod;
    type Burn = Burn;
    type BurnDestination = (); // Just gets burned.
    type WeightInfo = ();
    type SpendFunds = Bounties;
    type MaxApprovals = MaxApprovals;
}

impl committee::Config for TestRuntime {
    type Currency = Balances;
    type Event = Event;
    type Slash = Treasury;
    type DbcPrice = DBCPriceOCW;
}

parameter_types! {
    pub const CommitteeDuration: pallet_staking::EraIndex = 7;
}

impl lease_committee::Config for TestRuntime {
    type Event = Event;
    type Currency = Balances;
    type LCOperations = OnlineProfile;
    type ManageCommittee = Committee;
}

parameter_types! {
    pub const BlockPerEra: u32 = 3600 * 24 / 30;
    pub const BondingDuration: pallet_staking::EraIndex = 7;
    pub const ProfitReleaseDuration: u64 = 150;
}

impl online_profile::Config for TestRuntime {
    type Currency = Balances;
    type Event = Event;
    type BondingDuration = BondingDuration;
    type ProfitReleaseDuration = ProfitReleaseDuration;
    type Slash = Treasury;
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
    }
);

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = system::GenesisConfig::default().build_storage::<TestRuntime>().unwrap();

    #[rustfmt::skip]
    pallet_balances::GenesisConfig::<TestRuntime> {
        balances: vec![
            (sr25519::Public::from(Sr25519Keyring::Alice).into(), 1000_000),
            (sr25519::Public::from(Sr25519Keyring::Bob).into(), 1000_000),
            (sr25519::Public::from(Sr25519Keyring::Charlie).into(), 1000_000),
            (sr25519::Public::from(Sr25519Keyring::Dave).into(), 1000_000),
            (sr25519::Public::from(Sr25519Keyring::Eve).into(), 1000_000),
            (sr25519::Public::from(Sr25519Keyring::Ferdie).into(), 1000_000),
            (sr25519::Public::from(Sr25519Keyring::One).into(), 1000_000),
            (sr25519::Public::from(Sr25519Keyring::Two).into(), 1000_000),
        ],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    t.into()
}
