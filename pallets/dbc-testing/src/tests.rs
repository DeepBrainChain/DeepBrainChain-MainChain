use crate::{Config, Pallet};
use babe;
use dbc_staking as staking;
use frame_support::traits::PalletInfo;
use frame_support::{
    assert_noop, assert_ok, dispatch::DispatchError, impl_outer_origin, parameter_types,
};
use frame_system::{self as system, RawOrigin};
use pallet_balances as balances;
use sp_core::H256;
use sp_io::TestExternalities;
use sp_runtime::{
    testing::{Block, Header},
    traits::{BlakeTwo256, IdentityLookup},
    Perbill,
};

impl_outer_origin! {
    pub enum Origin for TestRuntime {}
}

// Workaround for https://github.com/rust-lang/rust/issues/26925 . Remove when sorted.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TestRuntime;
parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: u32 = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::one();
}
impl system::Config for TestRuntime {
    type BaseCallFilter = ();
    type Origin = Origin;
    type Index = u64;
    type Call = ();
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = ();
    type BlockHashCount = BlockHashCount;
    type DbWeight = ();
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = ();
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type BlockWeights = ();
    type BlockLength = ();
    type SS58Prefix = ();
}

impl babe::Config for TestRuntime {
    type EpochDuration = ();
    type ExpectedBlockTime = ();
    type EpochChangeTrigger = babe::ExternalTrigger;
    type KeyOwnerProof = ();
    type KeyOwnerIdentification = ();
    type KeyOwnerProofSystem = ();
    type HandleEquivocation = ();
    type WeightInfo = ();
}

// type KeyOwnerProofSystem = Historical;

// type KeyOwnerProof =
//     <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(KeyTypeId, AuthorityId)>>::Proof;

parameter_types! {
    pub const MinimumPeriod: u64 = 1;
}
impl pallet_timestamp::Config for TestRuntime {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

impl Config for TestRuntime {
    type Currency = Balances;
    type PhaseReward = Staking;
}

// 相当于添加了construct_runtime!
pub type System = system::Pallet<TestRuntime>;
pub type DBCTesting = Pallet<TestRuntime>;
pub type Staking = staking::Module<TestRuntime>;
pub type Balances = balances::Pallet<TestRuntime>;
// pub type HelloSubstrate = Pallet<TestRuntime>;

// type Block = frame_system::mocking::MockBlock<TestRuntime>;

struct ExternalityBuilder;

impl ExternalityBuilder {
    pub fn build() -> TestExternalities {
        let storage = system::GenesisConfig::default()
            .build_storage::<TestRuntime>()
            .unwrap();
        let mut ext = TestExternalities::from(storage);
        ext.execute_with(|| System::set_block_number(1));
        ext
    }
}

#[test]
fn say_hello_works() {
    ExternalityBuilder::build().execute_with(|| {
        assert_ok!(DBCTesting::say_hello(Origin::signed(1)));
    })
}

#[test]
fn say_hello_no_root() {
    ExternalityBuilder::build().execute_with(|| {
        assert_noop!(
            DBCTesting::say_hello(RawOrigin::Root.into()),
            DispatchError::BadOrigin
        );
    })
}
