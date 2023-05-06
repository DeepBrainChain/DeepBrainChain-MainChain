use crate as council_reward;
use frame_support::{
    parameter_types,
    traits::{ConstU32, LockIdentifier, OnFinalize, OnInitialize, U128CurrencyToVote},
    PalletId,
};
use frame_system::{EnsureRoot, EnsureWithSuccess};
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
    Permill,
};

// 初始1000WDBC
pub const INIT_BALANCE: u128 = 10_000_000 * ONE_DBC;
pub const DOLLARS: Balance = ONE_DBC / 100; // 10_000_000_000_000
pub const CENTS: Balance = DOLLARS / 100; // 100_000_000_000
pub const MILLICENTS: Balance = CENTS / 1_000; // 100_000_000

pub const fn deposit(items: u32, bytes: u32) -> Balance {
    // items as Balance * 15 * CENTS + (bytes as Balance) * 6 * CENTS
    items as Balance * 20 * DOLLARS + (bytes as Balance) * 100 * MILLICENTS
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;
type Balance = u128;
type BlockNumber = u64;
type Block = frame_system::mocking::MockBlock<TestRuntime>;

// 1 DBC = 1 * 10^15
pub const ONE_DBC: u128 = 1_000_000_000_000_000;
pub const ONE_DAY: BlockNumber = 2880;

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
    pub const MaxLocks :u32 = 50;
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

parameter_types! {
    pub const CouncilMotionDuration: u32 = 5 * 2880;
    pub const CouncilMaxProposals: u32 = 100;
    pub const CouncilMaxMembers: u32 = 100;
}

type CouncilCollective = pallet_collective::Instance1;
impl pallet_collective::Config<CouncilCollective> for TestRuntime {
    type RuntimeOrigin = RuntimeOrigin;
    type Proposal = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type MotionDuration = CouncilMotionDuration;
    type MaxProposals = CouncilMaxProposals;
    type MaxMembers = CouncilMaxMembers;
    type DefaultVote = pallet_collective::PrimeDefaultVote;
    type WeightInfo = pallet_collective::weights::SubstrateWeight<TestRuntime>;
}

parameter_types! {
    pub const CandidacyBond: Balance = 10000 * ONE_DBC;
    // 1 storage item created, key size is 32 bytes, value size is 16+16.
    pub const VotingBondBase: Balance = deposit(1, 64);
    // additional data per vote is 32 bytes (account id).
    pub const VotingBondFactor: Balance = deposit(0, 32);
    pub const TermDuration: BlockNumber = 120 * ONE_DAY;
    pub const DesiredMembers: u32 = 21;
    pub const DesiredRunnersUp: u32 = 7;
    pub const ElectionsPhragmenModuleId: LockIdentifier = *b"phrelect";
}

impl pallet_elections_phragmen::Config for TestRuntime {
    type RuntimeEvent = RuntimeEvent;
    type PalletId = ElectionsPhragmenModuleId;
    type Currency = Balances;
    type ChangeMembers = Council;
    // NOTE: this implies that council's genesis members cannot be set directly and must come from
    // this module.
    type InitializeMembers = Council;
    type CurrencyToVote = U128CurrencyToVote;
    type CandidacyBond = CandidacyBond;
    type VotingBondBase = VotingBondBase;
    type VotingBondFactor = VotingBondFactor;
    type LoserCandidate = ();
    type KickedMember = ();
    type DesiredMembers = DesiredMembers;
    type DesiredRunnersUp = DesiredRunnersUp;
    type TermDuration = TermDuration;
    type WeightInfo = pallet_elections_phragmen::weights::SubstrateWeight<TestRuntime>;
    type MaxCandidates = ();
    type MaxVoters = ();
}

impl dbc_price_ocw::Config for TestRuntime {
    type Currency = Balances;
    type RuntimeEvent = RuntimeEvent;
    type RandomnessSource = RandomnessCollectiveFlip;
}

parameter_types! {
    pub const BlockPerEra: u32 = 3600 * 24 / 30;
}

impl generic_func::Config for TestRuntime {
    type BlockPerEra = BlockPerEra;
    type Currency = Balances;
    type RuntimeEvent = RuntimeEvent;
    type RandomnessSource = RandomnessCollectiveFlip;
    type FixedTxFee = Treasury;
    type Slash = Treasury;
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

// 每月理事会首席发放 min(60万DBC/5000美金等值DBC)，理事会二三名发放min(20万DBC/2000美金等值DBC)，
parameter_types! {
    // 首要投票人：5000 USD 或 60万 DBC 取价值较小
    pub const PrimerReward: (u64, Balance) = (5_000_000_000u64, 600_000 * ONE_DBC);
    // 排名第二的议会成员：1000 USD 或 20万 DBC 取价值较小
    pub const SecondReward: (u64, Balance) = (2_000_000_000u64, 200_000 * ONE_DBC);
    // 排名第三的议会成员：1000 USD 或 20万 DBC 取价值较小
    pub const ThirdReward: (u64, Balance) = (2_000_000_000u64, 200_000 * ONE_DBC);
    // 发放周期
    pub const RewardFrequency: BlockNumber = 30 * ONE_DAY;
}

impl council_reward::Config for TestRuntime {
    type RuntimeEvent = RuntimeEvent;
    type DbcPrice = DBCPriceOCW;
    type Currency = Balances;
    type RewardFrequency = RewardFrequency;
    type PrimerReward = PrimerReward;
    type SecondReward = SecondReward;
    type ThirdReward = ThirdReward;
}

impl pallet_randomness_collective_flip::Config for TestRuntime {}

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum TestRuntime
    where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic, {
            System: frame_system,
            RandomnessCollectiveFlip: pallet_randomness_collective_flip,
            Balances: pallet_balances,
            DBCPriceOCW: dbc_price_ocw,
            Treasury: pallet_treasury,
            GenericFunc: generic_func,
            Council: pallet_collective::<Instance1>,
            Elections: pallet_elections_phragmen,
            CouncilReward: council_reward,
    }
);

pub fn new_test_ext_after_machine_online() -> sp_io::TestExternalities {
    let mut storage =
        frame_system::GenesisConfig::default().build_storage::<TestRuntime>().unwrap();

    #[rustfmt::skip]
    pallet_balances::GenesisConfig::<TestRuntime> {
        balances: vec![
            (sr25519::Public::from(Sr25519Keyring::Alice).into(), INIT_BALANCE),
            (sr25519::Public::from(Sr25519Keyring::Bob).into(), INIT_BALANCE),
            (sr25519::Public::from(Sr25519Keyring::Charlie).into(), INIT_BALANCE),
            (sr25519::Public::from(Sr25519Keyring::Dave).into(), 2 * INIT_BALANCE),
            (sr25519::Public::from(Sr25519Keyring::Eve).into(), INIT_BALANCE),
            (sr25519::Public::from(Sr25519Keyring::Ferdie).into(), INIT_BALANCE),
            (sr25519::Public::from(Sr25519Keyring::One).into(), INIT_BALANCE),
            (sr25519::Public::from(Sr25519Keyring::Two).into(), 100 * INIT_BALANCE),
        ],
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    let mut ext = sp_io::TestExternalities::from(storage);

    ext.execute_with(|| {
        run_to_block(1);
    });
    ext
}

pub fn run_to_block(n: BlockNumber) {
    for b in System::block_number()..=n {
        System::on_finalize(b);
        RandomnessCollectiveFlip::on_finalize(b);
        Balances::on_finalize(b);
        Treasury::on_finalize(b);
        Council::on_finalize(b);
        Elections::on_finalize(b);
        DBCPriceOCW::on_finalize(b);
        GenericFunc::on_finalize(b);
        CouncilReward::on_finalize(b);

        System::set_block_number(b + 1);

        System::on_initialize(b + 1);
        RandomnessCollectiveFlip::on_initialize(b + 1);
        Balances::on_initialize(b);
        Treasury::on_initialize(b);
        Council::on_initialize(b);
        Elections::on_initialize(b);
        DBCPriceOCW::on_initialize(b);
        GenericFunc::on_initialize(b);
        CouncilReward::on_initialize(b);
    }
}
