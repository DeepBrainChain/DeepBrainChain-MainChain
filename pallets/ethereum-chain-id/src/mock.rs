#![cfg(test)]

use frame_support::{parameter_types, traits::ConstU32};
pub use frame_system::RawOrigin;
pub use sp_core::{
    sr25519::{self, Signature},
    H256,
};
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;
type Block = frame_system::mocking::MockBlock<TestRuntime>;

pub use crate as ethereum_chain_id;

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
    type AccountData = ();
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = SS58Prefix;
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
}

parameter_types! {
    pub const ExistentialDeposit: u64 = 1;
    pub const MaxLocks :u32 = 50;
    pub const MaxReservers: u32 = 50;
}

impl ethereum_chain_id::Config for TestRuntime {
    type RuntimeEvent = RuntimeEvent;
}

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum TestRuntime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system,
        EthereumChainId: ethereum_chain_id,
    }
);

pub fn new_test_with() -> sp_io::TestExternalities {
    let t = frame_system::GenesisConfig::default().build_storage::<TestRuntime>().unwrap();
    
    let mut t: sp_io::TestExternalities = t.into();

    t.execute_with(|| {
        System::set_block_number(1);
    });

    t
}
