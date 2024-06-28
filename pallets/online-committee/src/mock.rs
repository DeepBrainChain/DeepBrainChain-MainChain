use crate as online_committee;
use dbc_price_ocw::MAX_LEN;
use dbc_support::machine_type::{
    CommitteeUploadInfo, Latitude, Longitude, StakerCustomizeInfo, StandardGpuPointPrice,
};
use frame_support::{
    assert_ok, parameter_types,
    traits::{ConstU32, OnFinalize, OnInitialize},
    PalletId,
};
pub use frame_system::RawOrigin;
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
    Perbill, Permill,
};
use std::convert::TryInto;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;
type Block = frame_system::mocking::MockBlock<TestRuntime>;
type Balance = u128;

// 1 DBC = 1 * 10^15
pub const ONE_DBC: u128 = 1_000_000_000_000_000;
// 初始1000WDBC
pub const INIT_BALANCE: u128 = 10_000_000 * ONE_DBC;
pub type BlockNumber = u64;
pub const INIT_TIMESTAMP: u64 = 90_000;
pub const BLOCK_TIME: u64 = 30_000;

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
    pub const MaxReservers: u32 = 50;
    pub const ExistentialDeposit: u64 = 1;
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

impl pallet_insecure_randomness_collective_flip::Config for TestRuntime {}

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

type TechnicalCollective = pallet_collective::Instance2;
impl pallet_collective::Config<TechnicalCollective> for TestRuntime {
    type RuntimeOrigin = RuntimeOrigin;
    type Proposal = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type MotionDuration = CouncilMotionDuration;
    type MaxProposals = CouncilMaxProposals;
    type MaxMembers = CouncilMaxMembers;
    type DefaultVote = pallet_collective::PrimeDefaultVote;
    type WeightInfo = pallet_collective::weights::SubstrateWeight<TestRuntime>;
    type SetMembersOrigin = EnsureRoot<Self::AccountId>;
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
    type RuntimeEvent = RuntimeEvent;
    // type WeightInfo = ();
}

impl online_committee::Config for TestRuntime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type OCOps = OnlineProfile;
    type ManageCommittee = Committee;
    type CancelSlashOrigin =
        pallet_collective::EnsureProportionAtLeast<Self::AccountId, TechnicalCollective, 2, 3>;
    type SlashAndReward = GenericFunc;
}

parameter_types! {
    pub const BondingDuration: u32 = 7;
    pub const ProfitReleaseDuration: u64 = 150;
}

impl online_profile::Config for TestRuntime {
    type Currency = Balances;
    type RuntimeEvent = RuntimeEvent;
    type BondingDuration = BondingDuration;
    type DbcPrice = DBCPriceOCW;
    type ManageCommittee = Committee;
    type Slash = Treasury;
    type CancelSlashOrigin =
        pallet_collective::EnsureProportionAtLeast<Self::AccountId, TechnicalCollective, 2, 3>;
    type SlashAndReward = GenericFunc;
}

#[allow(dead_code)]
fn key_to_pair(key: &str) -> sp_core::sr25519::Public {
    sr25519::Public::from_raw(hex::decode(key).unwrap().try_into().unwrap())
}

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum TestRuntime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system,
        Timestamp: pallet_timestamp,
        OnlineCommittee: online_committee,
        OnlineProfile: online_profile,
        RandomnessCollectiveFlip: pallet_insecure_randomness_collective_flip,
        Balances: pallet_balances,
        Committee: committee,
        DBCPriceOCW: dbc_price_ocw,
        Treasury: pallet_treasury,
        GenericFunc: generic_func,
        TechnicalCommittee: pallet_collective::<Instance2>,
    }
);

pub fn run_to_block(n: BlockNumber) {
    for b in System::block_number()..=n {
        // 当前块结束
        OnlineCommittee::on_finalize(b);
        OnlineProfile::on_finalize(b);
        Committee::on_finalize(b);
        System::on_finalize(b);
        RandomnessCollectiveFlip::on_finalize(b);
        Timestamp::set_timestamp(System::block_number() * BLOCK_TIME + INIT_TIMESTAMP);

        System::set_block_number(b + 1);

        // 下一块初始化
        RandomnessCollectiveFlip::on_initialize(b + 1);
        System::on_initialize(b + 1);
        OnlineCommittee::on_initialize(b + 1);
        Committee::on_initialize(b + 1);
        OnlineProfile::on_initialize(b + 1);
        RandomnessCollectiveFlip::on_initialize(b + 1);
        GenericFunc::on_initialize(b + 1);
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
        Timestamp::set_timestamp(System::block_number() * 30000 + INIT_TIMESTAMP);
        // 初始化设置参数
        // 委员会每次抢单质押数量 (1000 DBC)
        assert_ok!(Committee::set_committee_stake_params(
            RawOrigin::Root.into(),
            committee::CommitteeStakeParamsInfo {
                stake_baseline: 20000 * ONE_DBC,
                stake_per_order: 1000 * ONE_DBC,
                min_free_stake_percent: Perbill::from_rational(40u32, 100u32),
            },
        ));
        // 操作时的固定费率: 10 DBC
        assert_ok!(GenericFunc::set_fixed_tx_fee(RawOrigin::Root.into(), 10 * ONE_DBC));
        // 每张GPU质押数量: 100,000 DBC
        // 设置单卡质押上限： 7700_000_000, 每张GPU质押数量: 100,000 DBC
        assert_ok!(OnlineProfile::set_online_stake_params(
            RawOrigin::Root.into(),
            online_profile::OnlineStakeParamsInfo {
                online_stake_per_gpu: 100000 * ONE_DBC,
                online_stake_usd_limit: 7_700_000_000,
                // 设置重新上线绑定的金额: 47美元；这里为了方便计算，设置为24美元
                // 等值2000DBC
                reonline_stake: 24_000_000,
                slash_review_stake: 1000 * ONE_DBC,
            },
        ));
        // 设置奖励发放开始时间
        // 设置每个Era奖励数量: 1,100,000
        assert_ok!(OnlineProfile::set_reward_info(
            RawOrigin::Root.into(),
            online_profile::PhaseRewardInfoDetail {
                online_reward_start_era: 0,
                first_phase_duration: 1095,
                galaxy_on_era: 0,
                phase_0_reward_per_era: 1_100_000 * ONE_DBC,
                phase_1_reward_per_era: 550_000 * ONE_DBC,
                phase_2_reward_per_era: 275_000 * ONE_DBC,
            },
        ));

        // 设置标准GPU租金价格: (3080得分1000；租金每月1000RMB) {1000; 150_000_000};
        assert_ok!(OnlineProfile::set_standard_gpu_point_price(
            RawOrigin::Root.into(),
            StandardGpuPointPrice { gpu_point: 100, gpu_price: 28229 },
        ));

        // Set: Price URL: https://dbchaininfo.congtu.cloud/query/dbc_info?language=CN
        // 初始化price_ocw (0.012$)
        assert_eq!(DBCPriceOCW::avg_price(), None);
        for _ in 0..MAX_LEN {
            DBCPriceOCW::add_price(12_000u64);
        }
        DBCPriceOCW::add_avg_price();
        run_to_block(2);
    });

    // storage.into()
    ext
}

// 初始条件：设置了初始参数，并且已经分配给了3个委员会
// 测试惩罚机制
pub fn new_test_with_online_machine_distribution() -> sp_io::TestExternalities {
    let mut ext = new_test_with_init_params_ext();
    ext.execute_with(|| {
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

        let controller = sr25519::Public::from(Sr25519Keyring::Eve);
        let stash = sr25519::Public::from(Sr25519Keyring::Ferdie);
        // Bob pubkey
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();
        let msg = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48\
                   5CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL";
        let sig = "b4084f70730b183127e9db78c6d8dcf79039f23466cd1ee8b536c40c3027a83d\
                   ab040be4ed2db57b67eaac406817a69ce72a13f8ac11ba460e15d318b1504481";

        // stash 账户设置控制账户
        assert_ok!(OnlineProfile::set_controller(RuntimeOrigin::signed(stash), controller));

        // controller 生成server_name
        assert_ok!(OnlineProfile::gen_server_room(RuntimeOrigin::signed(controller)));
        assert_ok!(OnlineProfile::gen_server_room(RuntimeOrigin::signed(controller)));
        let server_room = OnlineProfile::stash_server_rooms(&stash);

        assert_ok!(OnlineProfile::bond_machine(
            RuntimeOrigin::signed(controller),
            machine_id.clone(),
            msg.as_bytes().to_vec(),
            hex::decode(sig).unwrap()
        ));

        run_to_block(5);

        // 控制账户添加机器信息
        assert_ok!(OnlineProfile::add_machine_info(
            RuntimeOrigin::signed(controller),
            machine_id,
            StakerCustomizeInfo {
                server_room: server_room[0],
                upload_net: 10000,
                download_net: 10000,
                longitude: Longitude::East(1157894),
                latitude: Latitude::North(235678),
                telecom_operators: vec!["China Unicom".into()],
                is_bare_machine: false
            }
        ));

        run_to_block(10);
    });
    ext
}

// 初始条件：机器已经成功上线，最新块高12
// 测试主动报告下线
pub fn new_test_with_machine_online() -> sp_io::TestExternalities {
    let mut ext = new_test_with_online_machine_distribution();

    let committee1 = sr25519::Public::from(Sr25519Keyring::Alice);
    let _committee2 = sr25519::Public::from(Sr25519Keyring::Charlie);
    let committee3 = sr25519::Public::from(Sr25519Keyring::Dave);
    let committee4 = sr25519::Public::from(Sr25519Keyring::Eve);

    let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
        .as_bytes()
        .to_vec();

    let machine_info_hash1: [u8; 16] =
        hex::decode("fd8885a22a9d9784adaa36effcd77522").unwrap().try_into().unwrap();
    let machine_info_hash2: [u8; 16] =
        hex::decode("c016090e0943c17f5d4999dc6eb52683").unwrap().try_into().unwrap();
    let machine_info_hash3: [u8; 16] =
        hex::decode("4a6b2df1e1a77b9bcdab5e31dc7950d2").unwrap().try_into().unwrap();

    ext.execute_with(|| {
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            RuntimeOrigin::signed(committee1),
            machine_id.clone(),
            machine_info_hash1
        ));
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            RuntimeOrigin::signed(committee3),
            machine_id.clone(),
            machine_info_hash2
        ));

        assert_ok!(OnlineCommittee::submit_confirm_hash(
            RuntimeOrigin::signed(committee4),
            machine_id.clone(),
            machine_info_hash3
        ));

        let mut committee_upload_info = CommitteeUploadInfo {
            machine_id: machine_id.clone(),
            gpu_type: "GeForceRTX3080".as_bytes().to_vec(),
            gpu_num: 4,
            cuda_core: 8704,
            gpu_mem: 10,
            calc_point: 59890,
            sys_disk: 500,
            data_disk: 3905,
            cpu_type: "Intel(R) Xeon(R) Silver 4214R".as_bytes().to_vec(),
            cpu_core_num: 46,
            cpu_rate: 2400,
            mem_num: 440,

            rand_str: "abcdefg1".as_bytes().to_vec(),
            is_support: true,
        };

        assert_ok!(OnlineCommittee::submit_confirm_raw(
            RuntimeOrigin::signed(committee1),
            committee_upload_info.clone()
        ));
        committee_upload_info.rand_str = "abcdefg2".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            RuntimeOrigin::signed(committee3),
            committee_upload_info.clone()
        ));
        committee_upload_info.rand_str = "abcdefg3".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            RuntimeOrigin::signed(committee4),
            committee_upload_info
        ));

        run_to_block(12);
    });
    ext
}
