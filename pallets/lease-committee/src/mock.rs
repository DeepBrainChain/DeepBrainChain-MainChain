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
    testing::{Header, TestXt},
    traits::{BlakeTwo256, IdentityLookup, Verify},
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;
type Block = frame_system::mocking::MockBlock<TestRuntime>;

type TestExtrinsic = TestXt<Call, ()>;

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

// impl lease_committee::Config for TestRuntime {
//     type Event = Event;
// }

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

parameter_types! {
    pub const CommitteeDuration: pallet_staking::EraIndex = 7;
}

impl<LocalCall> system::offchain::CreateSignedTransaction<LocalCall> for TestRuntime
where
    Call: From<LocalCall>,
{
    fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
        call: Call,
        _public: <Signature as Verify>::Signer,
        _account: <TestRuntime as system::Config>::AccountId,
        index: <TestRuntime as system::Config>::Index,
    ) -> Option<(
        Call,
        <TestExtrinsic as sp_runtime::traits::Extrinsic>::SignaturePayload,
    )> {
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

impl lease_committee::Config for TestRuntime {
    type Currency = Balances;
    type Event = Event;
    type CommitteeMachine = OnlineProfile;
    type CommitteeDuration = CommitteeDuration;
}

parameter_types! {
    pub const BlockPerEra: u32 = 3600 * 24 / 30;
    pub const BondingDuration: pallet_staking::EraIndex = 7;
}

impl online_profile::Config for TestRuntime {
    type Currency = Balances;
    type Event = Event;
    type RandomnessSource = RandomnessCollectiveFlip;
    type BlockPerEra = BlockPerEra;
    type BondingDuration = BondingDuration;
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
        OnlineProfile: online_profile::{Module, Call, Storage, Event<T>, ValidateUnsigned},
        RandomnessCollectiveFlip: pallet_randomness_collective_flip::{Module, Call, Storage},
        Balances: pallet_balances::{Module, Call, Storage, Event<T>},
    }
);

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = system::GenesisConfig::default()
        .build_storage::<TestRuntime>()
        .unwrap();

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
