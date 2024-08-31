use crate as rent_dlc_machine;
use dbc_price_ocw::MAX_LEN;
use dbc_support::machine_type::{
    CommitteeUploadInfo, Latitude, Longitude, StakerCustomizeInfo, StandardGpuPointPrice,
};

use frame_support::{
    assert_ok, parameter_types,
    traits::{AsEnsureOriginWithArg, ConstU128, ConstU32, OnFinalize, OnInitialize},
    weights::Weight,
    PalletId,
};
use frame_system::{EnsureRoot, EnsureSigned, EnsureWithSuccess, RawOrigin};
use parity_scale_codec::Compact;
pub use sp_core::{
    sr25519::{self, Signature},
    H256,
};
pub use sp_keyring::sr25519::Keyring as Sr25519Keyring;
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
pub const ONE_DLC: u128 = 1_000_000_000_000_000;

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

    type FreezeIdentifier = ();
    type MaxFreezes = ();
    type HoldIdentifier = ();
    type MaxHolds = ConstU32<2>;
}

impl pallet_insecure_randomness_collective_flip::Config for TestRuntime {}

parameter_types! {
    pub const BlockPerEra: u32 = 3600 * 24 / 30;
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
    pub BlockWeights: frame_system::limits::BlockWeights = frame_system::limits::BlockWeights::simple_max(Weight::MAX);
    pub MaxProposalWeight: Weight = sp_runtime::Perbill::from_percent(50) * BlockWeights::get().max_block;
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

    type MaxProposalWeight = MaxProposalWeight;
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

parameter_types! {
    pub const BondingDuration: u32 = 7;
    pub const ProfitReleaseDuration: u64 = 150;
}

impl committee::Config for TestRuntime {
    type Currency = Balances;
    type RuntimeEvent = RuntimeEvent;
    // type WeightInfo = ();
}

impl online_profile::Config for TestRuntime {
    type Currency = Balances;
    type RuntimeEvent = RuntimeEvent;
    type BondingDuration = BondingDuration;
    // type ProfitReleaseDuration = ProfitReleaseDuration;
    type DbcPrice = DBCPriceOCW;
    type ManageCommittee = Committee;
    type Slash = Treasury;
    type CancelSlashOrigin =
        pallet_collective::EnsureProportionAtLeast<Self::AccountId, TechnicalCollective, 2, 3>;
    type SlashAndReward = GenericFunc;
}

impl dbc_price_ocw::Config for TestRuntime {
    type Currency = Balances;
    type RuntimeEvent = RuntimeEvent;
    type RandomnessSource = RandomnessCollectiveFlip;
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

impl rent_machine::Config for TestRuntime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type RTOps = OnlineProfile;
    type DbcPrice = DBCPriceOCW;
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

impl generic_func::Config for TestRuntime {
    type Currency = Balances;
    type RuntimeEvent = RuntimeEvent;
    type RandomnessSource = RandomnessCollectiveFlip;
    type FixedTxFee = Treasury;
    type Slash = Treasury;
}

impl rent_dlc_machine::Config for TestRuntime {
    type RuntimeEvent = RuntimeEvent;
    type RTOps = OnlineProfile;
    type DbcPrice = DBCPriceOCW;
    type AssetId = u32;
    type DLCAssetId = ConstU32<88>;
}

impl dlc_machine::Config for TestRuntime {
    type RuntimeEvent = RuntimeEvent;
}
pub const DBCS: Balance = 1_000_000_000_000_000;
pub const DOLLARS: Balance = DBCS / 100;

parameter_types! {
    pub const AssetDeposit: Balance = 100 * DOLLARS;
    pub const ApprovalDeposit: Balance = 1 * DOLLARS;
    pub const StringLimit: u32 = 50;
    pub const MetadataDepositBase: Balance = 10 * DOLLARS;
    pub const MetadataDepositPerByte: Balance = 1 * DOLLARS;

    pub const AssetLockLimit: u32 = 1000;
    pub const MinLockAmount: Balance = 1 * 100_000_000;
    pub const MaxLockDuration: BlockNumber  = 10 *60 *24 *365;
}
impl pallet_assets::Config for TestRuntime {
    type RuntimeEvent = RuntimeEvent;
    type Balance = u128;
    type AssetId = u32;
    type AssetIdParameter = Compact<u32>;
    type Currency = Balances;
    type CreateOrigin =
        AsEnsureOriginWithArg<EnsureSigned<<TestRuntime as frame_system::Config>::AccountId>>;
    type ForceOrigin = EnsureRoot<<TestRuntime as frame_system::Config>::AccountId>;
    type AssetDeposit = AssetDeposit;
    type AssetAccountDeposit = ConstU128<DOLLARS>;
    type MetadataDepositBase = MetadataDepositBase;
    type MetadataDepositPerByte = MetadataDepositPerByte;
    type ApprovalDeposit = ApprovalDeposit;
    type StringLimit = StringLimit;
    type Freezer = ();
    type Extra = ();
    type CallbackHandle = ();
    type WeightInfo = pallet_assets::weights::SubstrateWeight<TestRuntime>;
    type RemoveItemsLimit = ConstU32<1000>;

    type MinLockAmount = MinLockAmount;

    type MaxLockDuration = MaxLockDuration;
    // #[cfg(feature = "runtime-benchmarks")]
    // type BenchmarkHelper = ();
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
        Assets: pallet_assets,
        RentMachine: rent_machine,
        TechnicalCommittee: pallet_collective::<Instance2>,
        RentDlcMachine: rent_dlc_machine,
        DLCMachine: dlc_machine,
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
            (sr25519::Public::from(Sr25519Keyring::Two).into(), INIT_BALANCE),
        ],
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    let mut ext = sp_io::TestExternalities::from(storage);

    let controller = sr25519::Public::from(Sr25519Keyring::Eve);
    let stash = sr25519::Public::from(Sr25519Keyring::Ferdie);
    let pot_two = sr25519::Public::from(Sr25519Keyring::Two);
    let committee1 = sr25519::Public::from(Sr25519Keyring::One);
    let committee2 = sr25519::Public::from(Sr25519Keyring::Two);
    let committee3 = sr25519::Public::from(Sr25519Keyring::Dave);

    // Bob pubkey
    let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
        .as_bytes()
        .to_vec();
    let msg = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48\
                   5CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL";
    let sig = "b4084f70730b183127e9db78c6d8dcf79039f23466cd1ee8b536c40c3027a83d\
                   ab040be4ed2db57b67eaac406817a69ce72a13f8ac11ba460e15d318b1504481";

    ext.execute_with(|| {
        run_to_block(1);
        // 初始化price_ocw (0.012$)
        assert_eq!(DBCPriceOCW::avg_price(), None);
        for _ in 0..MAX_LEN {
            DBCPriceOCW::add_price(12_000u64);
        }
        DBCPriceOCW::add_avg_price();
        assert_eq!(DBCPriceOCW::avg_price(), Some(12_000u64));

        // 初始化设置参数
        // 委员会每次抢单质押数量 (15$)
        // 委员会每次抢单质押数量 (15$)
        let _ = Committee::set_committee_stake_params(
            RawOrigin::Root.into(),
            committee::CommitteeStakeParamsInfo {
                stake_baseline: 20000 * ONE_DBC,
                stake_per_order: 1000 * ONE_DBC,
                min_free_stake_percent: Perbill::from_rational(40u32, 100u32),
            },
        );
        // assert_ok!(Committee::set_staked_usd_per_order(RawOrigin::Root.into(), 15_000_000));
        // 操作时的固定费率: 10 DBC
        assert_ok!(GenericFunc::set_fixed_tx_fee(RawOrigin::Root.into(), 10 * ONE_DBC));
        // 每张GPU质押数量: 100,000 DBC
        // 设置单卡质押上限： 7700_000_000, 每张GPU质押数量: 100,000 DBC
        let _ = OnlineProfile::set_online_stake_params(
            RawOrigin::Root.into(),
            online_profile::OnlineStakeParamsInfo {
                online_stake_per_gpu: 100000 * ONE_DBC,
                online_stake_usd_limit: 7700_000_000,
                reonline_stake: 46_000_000,
                slash_review_stake: 1000 * ONE_DBC,
            },
        );
        // 设置奖励发放开始时间
        // 设置每个Era奖励数量: 1,100,000
        let _ = OnlineProfile::set_reward_info(
            RawOrigin::Root.into(),
            online_profile::PhaseRewardInfoDetail {
                online_reward_start_era: 0,
                first_phase_duration: 1095,
                galaxy_on_era: 0,
                phase_0_reward_per_era: 1_100_000 * ONE_DBC,
                phase_1_reward_per_era: 550_000 * ONE_DBC,
                phase_2_reward_per_era: 275_000 * ONE_DBC,
            },
        );

        // 设置标准GPU租金价格: (3080得分1000；租金每月1000RMB) {1000; 150_000_000};
        assert_ok!(OnlineProfile::set_standard_gpu_point_price(
            RawOrigin::Root.into(),
            StandardGpuPointPrice { gpu_point: 1000, gpu_price: 5_000_000 }
        ));
        // 设置机器租金支付地址
        assert_ok!(RentMachine::set_rent_fee_pot(RawOrigin::Root.into(), pot_two));

        run_to_block(2);

        // stash 账户设置控制账户
        assert_ok!(OnlineProfile::set_controller(RuntimeOrigin::signed(stash), controller));

        // controller 生成server_name
        assert_ok!(OnlineProfile::gen_server_room(RuntimeOrigin::signed(controller)));
        assert_ok!(OnlineProfile::gen_server_room(RuntimeOrigin::signed(controller)));

        let server_room = OnlineProfile::stash_server_rooms(&stash);

        // controller bond_machine
        assert_ok!(OnlineProfile::bond_machine(
            RuntimeOrigin::signed(controller),
            machine_id.clone(),
            msg.as_bytes().to_vec(),
            hex::decode(sig).unwrap()
        ));

        // 控制账户添加机器信息
        assert_ok!(OnlineProfile::add_machine_info(
            RuntimeOrigin::signed(controller),
            machine_id.clone(),
            StakerCustomizeInfo {
                // server_room: H256::from_low_u64_be(1),
                server_room: server_room[0],
                upload_net: 10000,
                download_net: 10000,
                longitude: Longitude::East(1157894),
                latitude: Latitude::North(235678),
                telecom_operators: vec!["China Unicom".into()],
                is_bare_machine: false
            }
        ));

        run_to_block(3);

        // 增加一个委员会
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee1));
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee2));
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee3));

        let one_box_pubkey: [u8; 32] =
            hex::decode("9dccbab2d61405084eac440f877a6479bc827373b2e414e81a6170ebe5aadd12")
                .unwrap()
                .try_into()
                .unwrap();
        assert_ok!(Committee::committee_set_box_pubkey(
            RuntimeOrigin::signed(committee1),
            one_box_pubkey.clone()
        ));
        assert_ok!(Committee::committee_set_box_pubkey(
            RuntimeOrigin::signed(committee2),
            one_box_pubkey.clone()
        ));
        assert_ok!(Committee::committee_set_box_pubkey(
            RuntimeOrigin::signed(committee3),
            one_box_pubkey.clone()
        ));

        run_to_block(5);

        // 委员会提交机器Hash
        let machine_info_hash1: [u8; 16] =
            hex::decode("fd8885a22a9d9784adaa36effcd77522").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            RuntimeOrigin::signed(committee1),
            machine_id.clone(),
            machine_info_hash1
        ));
        let machine_info_hash2: [u8; 16] =
            hex::decode("c016090e0943c17f5d4999dc6eb52683").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            RuntimeOrigin::signed(committee2),
            machine_id.clone(),
            machine_info_hash2
        ));
        let machine_info_hash3: [u8; 16] =
            hex::decode("4a6b2df1e1a77b9bcdab5e31dc7950d2").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            RuntimeOrigin::signed(committee3),
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

        // 委员会提交原始信息
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            RuntimeOrigin::signed(committee1),
            committee_upload_info.clone()
        ));
        committee_upload_info.rand_str = "abcdefg2".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            RuntimeOrigin::signed(committee2),
            committee_upload_info.clone()
        ));
        committee_upload_info.rand_str = "abcdefg3".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            RuntimeOrigin::signed(committee3),
            committee_upload_info.clone()
        ));

        run_to_block(10);

        let asset_id = RentDlcMachine::get_dlc_asset_id_parameter();
        assert_ok!(Assets::create(RuntimeOrigin::signed(committee3), asset_id, committee3, 1));
        assert_ok!(Assets::mint(
            RuntimeOrigin::signed(committee3),
            asset_id,
            committee3,
            10_000_000 * ONE_DLC
        ));
    });

    ext
}

pub fn run_to_block(n: BlockNumber) {
    for b in System::block_number()..=n {
        OnlineProfile::on_finalize(b);
        OnlineCommittee::on_finalize(b);
        Committee::on_finalize(b);
        RentMachine::on_finalize(b);
        System::on_finalize(b);
        RandomnessCollectiveFlip::on_finalize(b);
        Timestamp::set_timestamp(System::block_number() * BLOCK_TIME + INIT_TIMESTAMP);
        RentDlcMachine::on_finalize(b);

        System::set_block_number(b + 1);

        System::on_initialize(b + 1);
        OnlineCommittee::on_initialize(b + 1);
        Committee::on_initialize(b + 1);
        OnlineProfile::on_initialize(b + 1);
        RandomnessCollectiveFlip::on_initialize(b + 1);
    }
}
