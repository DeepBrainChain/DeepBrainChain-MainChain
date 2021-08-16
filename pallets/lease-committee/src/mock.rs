use crate as lease_committee;
use dbc_price_ocw::MAX_LEN;
use frame_support::{
    assert_ok, parameter_types,
    traits::{OnFinalize, OnInitialize},
};
use frame_system::EnsureRoot;
pub use frame_system::RawOrigin;
use online_profile::{StakerCustomizeInfo, StandardGpuPointPrice};
pub use sp_core::{
    sr25519::{self, Signature},
    H256,
};
pub use sp_keyring::{ed25519::Keyring as Ed25519Keyring, sr25519::Keyring as Sr25519Keyring, AccountKeyring};
use sp_runtime::{
    testing::{Header, TestXt},
    traits::{BlakeTwo256, IdentityLookup, Verify},
    ModuleId, Permill,
};
use std::convert::TryInto;

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

impl dbc_price_ocw::Config for TestRuntime {
    type Currency = Balances;
    type Event = Event;
    type RandomnessSource = RandomnessCollectiveFlip;
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

impl committee::Config for TestRuntime {
    type Currency = Balances;
    type Event = Event;
    type Slash = Treasury;
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
    type DbcPrice = DBCPriceOCW;
    type ManageCommittee = Committee;
    type Slash = Treasury;
}

/// 待绑定的机器信息
pub struct MachineBondInfo<AccountId> {
    /// 控制账户
    pub controller_account: AccountId,
    /// stash账户
    pub stash_account: AccountId,
    /// 公钥
    pub machine_id: Vec<u8>,
    /// 私钥
    pub machine_key: Vec<u8>,
    /// sign_by_machine_key(machine_id + stash_account)
    pub machine_sig: Vec<u8>,
    /// committee upload info
    pub committee_submit: Vec<CommitteeSubmit<AccountId>>,
}

/// 委员会需要提交的机器信息
pub struct CommitteeSubmit<AccountId> {
    /// committee
    pub committee_account: AccountId,
    /// 委员会要提交的机器Hash
    pub machine_hash: Vec<u8>,
    /// 委员会要提交的机器信息
    pub machine_info: online_profile::CommitteeUploadInfo,
}

fn key_to_pair(key: &str) -> sp_core::sr25519::Public {
    sr25519::Public::from_raw(hex::decode(key).unwrap().try_into().unwrap())
}

pub fn gen_machine_online_info() -> Vec<MachineBondInfo<<TestRuntime as frame_system::Config>::AccountId>> {
    let machine_3080 = online_profile::CommitteeUploadInfo {
        gpu_type: "GeForceRTX3080".as_bytes().to_vec(),
        gpu_num: 4,
        cuda_core: 8704,
        gpu_mem: 10,
        calc_point: 35984,
        sys_disk: 500,
        data_disk: 20,
        cpu_type: "Intel(R) Xeon(R) Gold 6226R".as_bytes().to_vec(),
        cpu_core_num: 64,
        cpu_rate: 290,
        mem_num: 512000,

        is_support: true,

        ..Default::default()
    };

    // let machine_3090 = online_profile::CommitteeUploadInfo {
    //     gpu_type: "GeForceRTX3090".as_bytes().to_vec(),
    //     cuda_core: 10496,
    //     gpu_mem: 24,
    //     calc_point: 11545,
    //     ..machine_3080
    // };

    // 算工1，绑定3台机器
    let controller1 = "660340c001261548d41bdb5f106bdde292927f8db6a6673907307c6b4357c563";
    let stash1_key = "46275867654fc2284de52a29e259c3ce9706bb74182d3be461287770c746760e";
    let stash1_id = "5ECDefnZjPHacZB7vzzR5NiTgsEyUDPAQ5YSxd23Hx7mzKgU";

    let machine1_key = "a050196c144ef5906c95a35a0c88524ac911e79715fd278e2a29096015d244db";
    let machine1_id = "ccfb33e27f0050bc66b9b7299dc3461c09a028b8bf8ca56074a1150db1cb2335";
    let machine1_sig = "0x7c88afd9a05013d9031487316fbd482052215f4c7747fecc3218aff9cf099c2cb0851463dcbae3b798272a36edc4508c290ac24974e0dbb99c9adc29851cab8f";
    let mut machine1_info = machine_3080.clone();
    machine1_info.machine_id = machine1_id.as_bytes().to_vec();
    machine1_info.rand_str = "abcdefg1".as_bytes().to_vec();

    let machine2_key = "8ebf852758bb1f6dbe32527096ce57b21b779067ecc6e47d80e5a5ef0257194b";
    let machine2_id = "fac1867c8b8f07b64578fe9fd0cc4df1514a63447d227d7be0266892df7c8e2f";
    let machine2_sig = "0x14dc4944a0297217b90d3229682fe202a50d46d013cfb274b5e07b19698956537c8999c9ab49f8fc8f745294401540b6533eebe200d2a78eaf5183f94ebb068e";
    let mut machine2_info = machine_3080.clone();
    machine2_info.machine_id = machine2_id.as_bytes().to_vec();

    let machine3_key = "b7c228a3cac9d0852ffd811cca05aee088c704f88aac962d07218436a48bde94";
    let machine3_id = "6a2964d9bcbbce4516c44c4faf9f7c1e6d35d4b4d76a55537e7c53497aeb1118";
    let machine3_sig = "0xec6fbee52a2fddffa657b57b955e8dacc6302ba9760ed9a8c9cd9313d3630c4b7ee8c59840bb1b079736e6968d98072bc199633efafb30af362cbae020af7482";
    let mut machine3_info = machine_3080.clone();
    machine3_info.machine_id = machine3_id.as_bytes().to_vec();

    // 算工2，绑定2台机器
    let controller2 = "1f0211626765ffdcfd498cd76b5f9e60cc3fd7d6559a06478ba3bf2e7911eb69";
    let stash2_key = "46275867654fc2284de52a29e259c3ce9706bb74182d3be461287770c746760e";
    let stash2_id = "5ECDefnZjPHacZB7vzzR5NiTgsEyUDPAQ5YSxd23Hx7mzKgU";

    let machine4_key = "a30dea3224ea8f50f751c946c3ffc6acd04bdadefabbdd632394c4ee3d559955";
    let machine4_id = "74b9c56e2cb4457bc98ef00764276d0dd406d1374090fcf3efd357b07337ea03";
    let machine4_sig = "0x62d77ad887b09683b76b2cc5d8431d37f6fd3f001910c7e25d96f96b10153e42ac5ca826bd27c3b706346fe14ab4ac5c4adb6d4d571d1cdc8a260a23a2e3698a";
    let mut machine4_info = machine_3080.clone();
    machine4_info.machine_id = machine4_id.as_bytes().to_vec();

    let machine5_key = "4819dc030b3a847f72eda98a320b7c0a4dbb3e1bc4a2fcedb822774b329fba6c";
    let machine5_id = "d4123eef6e82a8ed8c3b5a5d63efd9496f86157c10e914915e477956217d8d7b";
    let machine5_sig = "0x6e274cd10e11cbe0a502da4546de0559bc7a9400cef7e590f53fef831aab933f9ed091adaf586733a746026b5c9995523826fef42fabfe3457b34b1360345f86";
    let mut machine5_info = machine_3080.clone();
    machine5_info.machine_id = machine5_id.as_bytes().to_vec();

    // 算工3， 绑定1台机器
    let controller3 = "e86416177c9df33dcb4c32a067cd99d80a08df0e4dd4f4ab15912eb60e0a3a81";
    let stash3_key = "17bba02c645a72595bef3f8e3cf6882329e239bd21434ba27ee6c5856d630b43";
    let stash3_id = "5ELSwBWgRq5jN1j2YaP7qWRPTri6pKHpfpd7H4AePziXBabx";

    let machine6_key = "d8605dceab1a01182e43ba9452d1e39dc4e89bc63a2a6012f8380a71ed62809e";
    let machine6_id = "143df6134aac849a446bc6a3460c2e06778161f3c0dc88cd299e358fd1e4232e";
    let machine6_sig = "0x2ca31781cc93c8dd1cd1e30f5b27bde2d7f0430557d82476d4b0f6ed679f270528a1c8f15aff85cb34a8c54dc6eb7755bf68392f5eb6deaf7c394793fdfb138c";
    let mut machine6_info = machine_3080.clone();
    machine6_info.machine_id = machine6_id.as_bytes().to_vec();

    // let machine_info1 = MachineBondInfo { controller_account: controller1 };

    // let machine_bond_info1 = MachineBondInfo {
    //     controller_account: key_to_pair(controller1),
    //     stash_account: key_to_pair(stash1_key),
    //     machine_id: machine1_id.as_bytes().to_vec(),
    //     machine_key: machine1_key.as_bytes().to_vec(),
    //     machine_sig: machine1_sig.as_bytes().to_vec(),
    //     committee_submit: vec![CommitteeSubmit {
    //         // committee_account: account1,
    //         ..Default::default()
    //     }],
    // };

    let out = Vec::new();

    out
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
    }
);

pub fn run_to_block(n: BlockNumber) {
    for b in System::block_number()..=n {
        // 当前块结束
        OnlineProfile::on_finalize(b);
        LeaseCommittee::on_finalize(b);
        Committee::on_finalize(b);
        System::on_finalize(b);
        RandomnessCollectiveFlip::on_finalize(b);

        System::set_block_number(b + 1);

        // 下一块初始化
        RandomnessCollectiveFlip::on_initialize(b + 1);
        System::on_initialize(b + 1);
        LeaseCommittee::on_initialize(b + 1);
        Committee::on_initialize(b + 1);
        OnlineProfile::on_initialize(b + 1);
        RandomnessCollectiveFlip::on_initialize(b + 1);
    }
}

// 初始条件：只设置初始参数
// Build genesis storage according to the mock runtime.
pub fn new_test_with_init_params_ext() -> sp_io::TestExternalities {
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

    ext.execute_with(|| {
        // 初始化设置参数
        // 委员会每次抢单质押数量 (15$)
        let _ = Committee::set_committee_stake_params(
            RawOrigin::Root.into(),
            committee::CommitteeStakeParamsInfo {
                stake_baseline: 20000 * ONE_DBC,
                stake_per_order: 1000 * ONE_DBC,
                min_free_stake: 8000 * ONE_DBC,
            },
        );
        // 操作时的固定费率: 10 DBC
        let _ = GenericFunc::set_fixed_tx_fee(RawOrigin::Root.into(), 10 * ONE_DBC);
        // 每张GPU质押数量: 100,000 DBC
        let _ = OnlineProfile::set_gpu_stake(RawOrigin::Root.into(), 100_000 * ONE_DBC);
        // 设置奖励发放开始时间
        let _ = OnlineProfile::set_reward_start_era(RawOrigin::Root.into(), 0);
        // 设置每个Era奖励数量: 1,100,000
        let _ = OnlineProfile::set_phase_n_reward_per_era(RawOrigin::Root.into(), 0, 1_100_000 * ONE_DBC);
        let _ = OnlineProfile::set_phase_n_reward_per_era(RawOrigin::Root.into(), 1, 1_100_000 * ONE_DBC);
        // 设置单卡质押上限： 7700_000_000
        let _ = OnlineProfile::set_stake_usd_limit(RawOrigin::Root.into(), 7700_000_000);
        // 设置标准GPU租金价格: (3080得分1000；租金每月1000RMB) {1000; 150_000_000};
        let _ = OnlineProfile::set_standard_gpu_point_price(
            RawOrigin::Root.into(),
            StandardGpuPointPrice { gpu_point: 1000, gpu_price: 5_000_000 },
        );

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
        let committee1: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Alice).into();
        let committee2: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Charlie).into();
        let committee3: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Dave).into();
        let committee4: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();

        // 增加四个委员会
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee1));
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee2));
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee3));
        assert_ok!(Committee::add_committee(RawOrigin::Root.into(), committee4));
        let committee1_box_pubkey = hex::decode("ff3033c763f71bc51f372c1dc5095accc26880e138df84cac13c46bfd7dbd74f")
            .unwrap()
            .try_into()
            .unwrap();
        let committee2_box_pubkey = hex::decode("336404f7d316565cc3c3350e70561f4177803e0bb02a7f2e4e02a4f0e361157e")
            .unwrap()
            .try_into()
            .unwrap();
        let committee3_box_pubkey = hex::decode("a7804e30caa5645e97489b2d4711e3d8f4e17a683338cba97a53b960648f0438")
            .unwrap()
            .try_into()
            .unwrap();
        let committee4_box_pubkey = hex::decode("5eec53877f4b18c8b003fa983d27ef2e5518b7e4d08d482922a7787f2ea75529")
            .unwrap()
            .try_into()
            .unwrap();

        assert_ok!(Committee::committee_set_box_pubkey(Origin::signed(committee1), committee1_box_pubkey));
        assert_ok!(Committee::committee_set_box_pubkey(Origin::signed(committee2), committee2_box_pubkey));
        assert_ok!(Committee::committee_set_box_pubkey(Origin::signed(committee3), committee3_box_pubkey));
        assert_ok!(Committee::committee_set_box_pubkey(Origin::signed(committee4), committee4_box_pubkey));

        let controller: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();
        let stash: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Ferdie).into();
        // Bob pubkey
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();
        let msg = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48\
                   5CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL";
        let sig = "3abb2adb1bad83b87d61be8e55c31cec4b3fb2ecc5ee7254c8df88b1ec92e025\
                   4f4a9b010e2d8a5cce9d262e9193b76be87b46f6bef4219517cf939520bfff84";

        // stash 账户设置控制账户
        assert_ok!(OnlineProfile::set_controller(Origin::signed(stash), controller));

        // controller 生成server_name
        assert_ok!(OnlineProfile::gen_server_room(Origin::signed(controller)));
        assert_ok!(OnlineProfile::gen_server_room(Origin::signed(controller)));

        let server_room = OnlineProfile::stash_server_rooms(&stash);

        assert_ok!(OnlineProfile::bond_machine(
            Origin::signed(controller),
            machine_id.clone(),
            msg.as_bytes().to_vec(),
            hex::decode(sig).unwrap()
        ));

        run_to_block(5);

        // 控制账户添加机器信息
        assert_ok!(OnlineProfile::add_machine_info(
            Origin::signed(controller),
            machine_id.clone(),
            StakerCustomizeInfo {
                server_room: server_room[0],
                upload_net: 10000,
                download_net: 10000,
                longitude: online_profile::Longitude::East(1157894),
                latitude: online_profile::Latitude::North(235678),
                telecom_operators: vec!["China Unicom".into()],
            }
        ));

        run_to_block(10);
    });
    ext
}
