use crate as rent_machine;
use dbc_price_ocw::MAX_LEN;
use frame_support::{
    assert_ok, parameter_types,
    traits::{OnFinalize, OnInitialize},
};
pub use frame_system::{self as system, RawOrigin};
pub use sp_core::{
    sr25519::{self, Signature},
    u32_trait::{_1, _2, _3, _4, _5},
    H256,
};
pub use sp_keyring::{ed25519::Keyring as Ed25519Keyring, sr25519::Keyring as Sr25519Keyring, AccountKeyring};
use sp_runtime::{
    testing::{Header, TestXt},
    traits::{BlakeTwo256, IdentityLookup, Verify},
    ModuleId, Perbill, Permill,
};
use std::convert::TryInto;

use frame_system::EnsureRoot;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;
type Block = frame_system::mocking::MockBlock<TestRuntime>;

// 1 DBC = 1 * 10^15
pub const ONE_DBC: u128 = 1_000_000_000_000_000;
// 初始1000WDBC
pub const INIT_BALANCE: u128 = 10_000_000 * ONE_DBC;
pub type BlockNumber = u64;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const SS58Prefix: u8 = 42;
}

impl frame_system::Config for TestRuntime {
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
    pub const CouncilMotionDuration: u32 = 5 * 2880;
    pub const CouncilMaxProposals: u32 = 100;
    pub const CouncilMaxMembers: u32 = 100;
}

type TechnicalCollective = pallet_collective::Instance2;
impl pallet_collective::Config<TechnicalCollective> for TestRuntime {
    type Origin = Origin;
    type Proposal = Call;
    type Event = Event;
    type MotionDuration = CouncilMotionDuration;
    type MaxProposals = CouncilMaxProposals;
    type MaxMembers = CouncilMaxMembers;
    type DefaultVote = pallet_collective::PrimeDefaultVote;
    type WeightInfo = pallet_collective::weights::SubstrateWeight<TestRuntime>;
}

parameter_types! {
    pub const BondingDuration: u32 = 7;
    pub const ProfitReleaseDuration: u64 = 150;
}

impl committee::Config for TestRuntime {
    type Currency = Balances;
    type Event = Event;
    type Slash = Treasury;
    type CancelSlashOrigin = pallet_collective::EnsureProportionAtLeast<_2, _3, Self::AccountId, TechnicalCollective>;
}

impl online_profile::Config for TestRuntime {
    type Currency = Balances;
    type Event = Event;
    type BondingDuration = BondingDuration;
    // type ProfitReleaseDuration = ProfitReleaseDuration;
    type DbcPrice = DBCPriceOCW;
    type ManageCommittee = Committee;
    type Slash = Treasury;
    type CancelSlashOrigin = pallet_collective::EnsureProportionAtLeast<_2, _3, Self::AccountId, TechnicalCollective>;
}

impl dbc_price_ocw::Config for TestRuntime {
    type Currency = Balances;
    type Event = Event;
    type RandomnessSource = RandomnessCollectiveFlip;
}

impl online_committee::Config for TestRuntime {
    type Event = Event;
    type Currency = Balances;
    type OCOperations = OnlineProfile;
    type ManageCommittee = Committee;
}

impl rent_machine::Config for TestRuntime {
    type Currency = Balances;
    type Event = Event;
    type RTOps = OnlineProfile;
    type DbcPrice = DBCPriceOCW;
}

type TestExtrinsic = TestXt<Call, ()>;
impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for TestRuntime
where
    Call: From<LocalCall>,
{
    fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
        call: Call,
        _public: <Signature as Verify>::Signer,
        _account: <TestRuntime as frame_system::Config>::AccountId,
        index: <TestRuntime as frame_system::Config>::Index,
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

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum TestRuntime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Module, Call, Config, Storage, Event<T>},
        OnlineCommittee: online_committee::{Module, Call, Storage, Event<T>},
        OnlineProfile: online_profile::{Module, Call, Storage, Event<T>},
        RandomnessCollectiveFlip: pallet_randomness_collective_flip::{Module, Call, Storage},
        Balances: pallet_balances::{Module, Call, Storage, Event<T>},
        Committee: committee::{Module, Call, Storage, Event<T>},
        DBCPriceOCW: dbc_price_ocw::{Module, Call, Storage, Event<T>, ValidateUnsigned},
        Treasury: pallet_treasury::{Module, Call, Storage, Config, Event<T>},
        GenericFunc: generic_func::{Module, Call, Storage, Event<T>},
        RentMachine: rent_machine::{Module, Storage, Call, Event<T>},
        TechnicalCommittee: pallet_collective::<Instance2>::{Module, Call, Storage, Origin<T>, Event<T>, Config<T>},
    }
);

pub fn new_test_ext_after_machine_online() -> sp_io::TestExternalities {
    let mut storage = frame_system::GenesisConfig::default().build_storage::<TestRuntime>().unwrap();

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

    let controller: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();
    let stash: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();
    let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();
    let one_committee: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One).into();
    let pot_two: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Two).into();
    let msg = "8eaf04151687736326c9fea17e25fc5287613693c912909\
                   cb226aa4794f26a485CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL";
    let sig = "3abb2adb1bad83b87d61be8e55c31cec4b3fb2ecc5ee7254c8df88b1ec92e02\
                   54f4a9b010e2d8a5cce9d262e9193b76be87b46f6bef4219517cf939520bfff84";

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
        Committee::set_committee_stake_params(
            RawOrigin::Root.into(),
            committee::CommitteeStakeParamsInfo {
                stake_baseline: 20000 * ONE_DBC,
                stake_per_order: 1000 * ONE_DBC,
                min_free_stake_percent: Perbill::from_rational_approximation(40u32, 100u32),
            },
        );
        // assert_ok!(Committee::set_staked_usd_per_order(RawOrigin::Root.into(), 15_000_000));
        // 操作时的固定费率: 10 DBC
        assert_ok!(GenericFunc::set_fixed_tx_fee(RawOrigin::Root.into(), 10 * ONE_DBC));
        // 每张GPU质押数量: 100,000 DBC
        assert_ok!(OnlineProfile::set_gpu_stake(RawOrigin::Root.into(), 100_000 * ONE_DBC));
        // 设置奖励发放开始时间
        assert_ok!(OnlineProfile::set_reward_start_era(RawOrigin::Root.into(), 0));
        // 设置每个Era奖励数量: 1,100,000
        assert_ok!(OnlineProfile::set_phase_n_reward_per_era(RawOrigin::Root.into(), 0, 1_100_000 * ONE_DBC));
        assert_ok!(OnlineProfile::set_phase_n_reward_per_era(RawOrigin::Root.into(), 1, 1_100_000 * ONE_DBC));
        // 设置单卡质押上限： 7700_000_000
        assert_ok!(OnlineProfile::set_stake_usd_limit(RawOrigin::Root.into(), 7700_000_000));
        // 设置标准GPU租金价格: (3080得分1000；租金每月1000RMB) {1000; 150_000_000};
        assert_ok!(OnlineProfile::set_standard_gpu_point_price(
            RawOrigin::Root.into(),
            online_profile::StandardGpuPointPrice { gpu_point: 1000, gpu_price: 5_000_000 }
        ));
        // 设置机器租金支付地址
        assert_ok!(RentMachine::set_rent_fee_pot(RawOrigin::Root.into(), pot_two));

        run_to_block(2);

        // stash 账户设置控制账户
        assert_ok!(OnlineProfile::set_controller(Origin::signed(stash), controller));

        // controller 生成server_name
        assert_ok!(OnlineProfile::gen_server_room(Origin::signed(controller)));
        assert_ok!(OnlineProfile::gen_server_room(Origin::signed(controller)));

        let server_room = OnlineProfile::stash_server_rooms(&stash);

        // controller bond_machine
        assert_ok!(OnlineProfile::bond_machine(
            Origin::signed(controller),
            machine_id.clone(),
            msg.as_bytes().to_vec(),
            hex::decode(sig).unwrap()
        ));

        // 控制账户添加机器信息
        assert_ok!(OnlineProfile::add_machine_info(
            Origin::signed(controller),
            machine_id.clone(),
            online_profile::StakerCustomizeInfo {
                // server_room: H256::from_low_u64_be(1),
                server_room: server_room[0],
                upload_net: 10000,
                download_net: 10000,
                longitude: online_profile::Longitude::East(1157894),
                latitude: online_profile::Latitude::North(235678),
                telecom_operators: vec!["China Unicom".into()],
            }
        ));

        run_to_block(3);

        // 增加一个委员会
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), one_committee));
        let one_box_pubkey = hex::decode("9dccbab2d61405084eac440f877a6479bc827373b2e414e81a6170ebe5aadd12")
            .unwrap()
            .try_into()
            .unwrap();
        assert_ok!(Committee::committee_set_box_pubkey(Origin::signed(one_committee), one_box_pubkey));

        run_to_block(5);

        // 委员会提交机器Hash
        let machine_info_hash = "d80b116fd318f19fd89da792aba5e875";
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(one_committee),
            machine_id.clone(),
            hex::decode(machine_info_hash).unwrap().try_into().unwrap()
        ));

        // 委员会提交原始信息
        assert_ok!(OnlineCommittee::submit_confirm_raw(
            Origin::signed(one_committee),
            online_profile::CommitteeUploadInfo {
                machine_id: machine_id.clone(),
                gpu_type: "GeForceRTX2080Ti".as_bytes().to_vec(),
                gpu_num: 4,
                cuda_core: 4352,
                gpu_mem: 11283456,
                calc_point: 6825,
                sys_disk: 12345465,
                data_disk: 324567733,
                cpu_type: "Intel(R) Xeon(R) Silver 4110 CPU".as_bytes().to_vec(),
                cpu_core_num: 32,
                cpu_rate: 26,
                mem_num: 527988672,

                rand_str: "abcdefg".as_bytes().to_vec(),
                is_support: true,
            }
        ));

        run_to_block(10);
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

        System::set_block_number(b + 1);

        System::on_initialize(b + 1);
        OnlineCommittee::on_initialize(b + 1);
        Committee::on_initialize(b + 1);
        OnlineProfile::on_initialize(b + 1);
        RandomnessCollectiveFlip::on_initialize(b + 1);
    }
}
