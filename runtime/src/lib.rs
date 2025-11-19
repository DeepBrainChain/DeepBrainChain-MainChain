// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! The Substrate runtime. This can be compiled with `#[no_std]`, ready for Wasm.

#![warn(unused_crate_dependencies)]
#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 512.
#![recursion_limit = "512"]

const TARGET: &'static str = "runtime";

// Fix `unused_crate_dependencies` warnings.
use dbc_evm_tracer as _;
pub use dbc_primitives::{
    AccountId, AccountIndex, Balance, BlockNumber, Hash, Index, Moment, Signature,
};
// Fix `unused_crate_dependencies` warnings.
use dbc_primitives_rpc_evm_tracing_events as _;
use dbc_primitives_rpc_txpool::TxPoolResponse;
use dbc_support::{rental_type::MachineGPUOrder, EraIndex, MachineId, RentOrderId};
use fp_evm::weight_per_gas;
use fp_rpc::TransactionStatus;
use frame_election_provider_support::{
    onchain, BalancingConfig, ElectionDataProvider, SequentialPhragmen, VoteWeight,
};
use frame_support::{
    construct_runtime,
    dispatch::DispatchClass,
    pallet_prelude::Get,
    parameter_types,
    traits::{
        AsEnsureOriginWithArg, ConstU128, ConstU16, ConstU32, Currency as CurrencyT,
        EitherOfDiverse, EqualPrivilegeOnly, Everything, Hooks, Imbalance, InstanceFilter,
        KeyOwnerProofSystem, LockIdentifier, OnUnbalanced, U128CurrencyToVote,
    },
    weights::{
        constants::{
            BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight, WEIGHT_REF_TIME_PER_SECOND,
        },
        ConstantMultiplier, IdentityFee, Weight,
    },
    PalletId, RuntimeDebug,
};
use frame_system::{
    limits::{BlockLength, BlockWeights},
    EnsureRoot, EnsureSigned, EnsureWithSuccess,
};
use pallet_balances::NegativeImbalance;
use pallet_election_provider_multi_phase::SolutionAccuracyOf;
use pallet_ethereum::{Call::transact, PostLogContent, Transaction as EthereumTransaction};
use pallet_evm::{
    Account as EVMAccount, AddressMapping, EVMCurrencyAdapter, EnsureAddressNever,
    EnsureAddressRoot, EvmConfig, FeeCalculator, GasWeightMapping, HashedAddressMapping,
    OnChargeEVMTransaction as OnChargeEVMTransactionT, Runner,
};
use pallet_grandpa::AuthorityId as GrandpaId;
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use pallet_nfts::PalletFeatures;
use pallet_session::historical::{self as pallet_session_historical};
pub use pallet_transaction_payment::{CurrencyAdapter, Multiplier, TargetedFeeAdjustment};
use pallet_transaction_payment::{FeeDetails, RuntimeDispatchInfo};
use parity_scale_codec::{Compact, Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_api::impl_runtime_apis;
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata, H160, H256, U256};
use sp_inherents::{CheckInherentsResult, InherentData};
use sp_runtime::{
    create_runtime_str,
    curve::PiecewiseLinear,
    generic, impl_opaque_keys,
    traits::{
        self, BlakeTwo256, Block as BlockT, Bounded, ConvertInto, DispatchInfoOf, Dispatchable,
        LookupError, NumberFor, OpaqueKeys, PostDispatchInfoOf, SaturatedConversion, StaticLookup,
        UniqueSaturatedInto,
    },
    transaction_validity::{
        TransactionPriority, TransactionSource, TransactionValidity, TransactionValidityError,
    },
    ApplyExtrinsicResult, FixedPointNumber, FixedU128, MultiAddress, PerThing, Perbill, Percent,
    Permill, Perquintill,
};

use sp_std::prelude::*;
#[cfg(any(feature = "std", test))]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;
use static_assertions::const_assert;

#[cfg(any(feature = "std", test))]
pub use frame_system::Call as SystemCall;
#[cfg(any(feature = "std", test))]
pub use pallet_balances::Call as BalancesCall;
#[cfg(any(feature = "std", test))]
pub use pallet_staking::StakerStatus;
#[cfg(any(feature = "std", test))]
pub use pallet_sudo::Call as SudoCall;
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;

/// Implementations of some helper traits passed into runtime modules as associated types.
pub mod impls;
use impls::CreditToBlockAuthor;

/// Constant values used within the runtime.
pub mod constants;
use constants::{currency::*, time::*};
use sp_runtime::generic::Era;

mod precompiles;
use precompiles::DBCPrecompiles;

mod migrations;

/// Generated voter bag information.
mod voter_bags;

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

/// Wasm binary unwrapped. If built with `SKIP_WASM_BUILD`, the function panics.
#[cfg(feature = "std")]
pub fn wasm_binary_unwrap() -> &'static [u8] {
    WASM_BINARY.expect(
        "Development wasm binary is not available. This means the client is built with \
		 `SKIP_WASM_BUILD` flag and it is only usable for production chains. Please rebuild with \
		 the flag disabled.",
    )
}

/// Runtime version.
#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: create_runtime_str!("DeepBrainChain"),
    impl_name: create_runtime_str!("DeepBrainChain"),
    authoring_version: 1,
    // Per convention: if the runtime behavior changes, increment spec_version
    // and set impl_version to 0. If only runtime
    // implementation changes and behavior does not, then leave spec_version as
    // is and increment impl_version.
    spec_version: 407,
    impl_version: 0,
    apis: RUNTIME_API_VERSIONS,
    transaction_version: 1,
    state_version: 1,
};

/// The BABE epoch configuration at genesis.
pub const BABE_GENESIS_EPOCH_CONFIG: sp_consensus_babe::BabeEpochConfiguration =
    sp_consensus_babe::BabeEpochConfiguration {
        c: PRIMARY_PROBABILITY,
        allowed_slots: sp_consensus_babe::AllowedSlots::PrimaryAndSecondaryPlainSlots,
    };

/// Native version.
#[cfg(any(feature = "std", test))]
pub fn native_version() -> NativeVersion {
    NativeVersion { runtime_version: VERSION, can_author_with: Default::default() }
}

pub struct DealWithFees<R>(sp_std::marker::PhantomData<R>);
impl<R> OnUnbalanced<NegativeImbalance<R>> for DealWithFees<R>
where
    R: pallet_balances::Config + pallet_treasury::Config + pallet_authorship::Config,
    pallet_treasury::Pallet<R>: OnUnbalanced<NegativeImbalance<R>>,
{
    // this seems to be called for substrate-based transactions
    fn on_unbalanceds<B>(mut fees_then_tips: impl Iterator<Item = NegativeImbalance<R>>) {
        if let Some(fees) = fees_then_tips.next() {
            // for fees, 80% to treasury, 20% to author
            let (to_treasury, to_author) = fees.ration(80, 20);

            <pallet_treasury::Pallet<R> as OnUnbalanced<_>>::on_unbalanced(to_treasury);

            if let Some(author) = pallet_authorship::Pallet::<R>::author() {
                pallet_balances::Pallet::<R>::resolve_creating(&author, to_author);
            }

            // handle tip if there is one
            if let Some(tip) = fees_then_tips.next() {
                // for now we use the same treasury/author strategy used for regular fees
                let (to_treasury, to_author) = tip.ration(80, 20);

                <pallet_treasury::Pallet<R> as OnUnbalanced<_>>::on_unbalanced(to_treasury);

                if let Some(author) = pallet_authorship::Pallet::<R>::author() {
                    pallet_balances::Pallet::<R>::resolve_creating(&author, to_author);
                }
            }
        }
    }

    // this is called from pallet_evm for Ethereum-based transactions
    // (technically, it calls on_unbalanced, which calls this when non-zero)
    fn on_nonzero_unbalanced(amount: NegativeImbalance<R>) {
        // for fees, 80% to treasury, 20% to author
        let (to_treasury, to_author) = amount.ration(80, 20);

        <pallet_treasury::Pallet<R> as OnUnbalanced<_>>::on_unbalanced(to_treasury);

        if let Some(author) = pallet_authorship::Pallet::<R>::author() {
            pallet_balances::Pallet::<R>::resolve_creating(&author, to_author);
        }
    }
}

/// We assume that ~10% of the block weight is consumed by `on_initialize` handlers.
/// This is used to limit the maximal weight of a single extrinsic.
const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(10);
/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be used
/// by  Operational  extrinsics.
const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);
/// We allow for 2 seconds of compute with a 6 second average block time, with maximum proof size.
const MAXIMUM_BLOCK_WEIGHT: Weight =
    Weight::from_parts(WEIGHT_REF_TIME_PER_SECOND.saturating_mul(2), u64::MAX);

parameter_types! {
    pub const BlockHashCount: BlockNumber = 2400;
    pub const Version: RuntimeVersion = VERSION;
    pub RuntimeBlockLength: BlockLength =
        BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
    pub RuntimeBlockWeights: BlockWeights = BlockWeights::builder()
        .base_block(BlockExecutionWeight::get())
        .for_class(DispatchClass::all(), |weights| {
            weights.base_extrinsic = ExtrinsicBaseWeight::get();
        })
        .for_class(DispatchClass::Normal, |weights| {
            weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
        })
        .for_class(DispatchClass::Operational, |weights| {
            weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
            // Operational transactions have some extra reserved space, so that they
            // are included even if block reached `MAXIMUM_BLOCK_WEIGHT`.
            weights.reserved = Some(
                MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT
            );
        })
        .avg_block_initialization(AVERAGE_ON_INITIALIZE_RATIO)
        .build_or_panic();
    pub MaxCollectivesProposalWeight: Weight = Perbill::from_percent(50) * RuntimeBlockWeights::get().max_block;
}

const_assert!(NORMAL_DISPATCH_RATIO.deconstruct() >= AVERAGE_ON_INITIALIZE_RATIO.deconstruct());

pub struct EvmAddressMapping;
impl StaticLookup for EvmAddressMapping {
    type Source = MultiAddress<AccountId, AccountIndex>;
    type Target = AccountId;

    fn lookup(a: Self::Source) -> Result<Self::Target, LookupError> {
        match a {
            MultiAddress::Address20(i) => {
                Ok(HashedAddressMapping::<BlakeTwo256>::into_account_id(H160::from(&i)).into())
            },
            _ => Err(LookupError),
        }
    }

    fn unlookup(a: Self::Target) -> Self::Source {
        MultiAddress::Id(a)
    }
}

impl frame_system::Config for Runtime {
    type BaseCallFilter = Everything;
    type BlockWeights = RuntimeBlockWeights;
    type BlockLength = RuntimeBlockLength;
    type DbWeight = RocksDbWeight;
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Index = Index;
    type BlockNumber = BlockNumber;
    type Hash = Hash;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = (Indices, EvmAddressMapping);
    type Header = generic::Header<BlockNumber, BlakeTwo256>;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = BlockHashCount;
    type Version = Version;
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = frame_system::weights::SubstrateWeight<Runtime>;
    type SS58Prefix = ConstU16<42>;
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
}

impl pallet_insecure_randomness_collective_flip::Config for Runtime {}

impl pallet_utility::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type PalletsOrigin = OriginCaller;
    type WeightInfo = pallet_utility::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    // One storage item; key size is 32; value is size 4+4+16+32 bytes = 56 bytes.
    pub const DepositBase: Balance = deposit(1, 88);
    // Additional storage item size of 32 bytes.
    pub const DepositFactor: Balance = deposit(0, 32);
}

impl pallet_multisig::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type Currency = Balances;
    type DepositBase = DepositBase;
    type DepositFactor = DepositFactor;
    type MaxSignatories = ConstU32<100>;
    type WeightInfo = pallet_multisig::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    // One storage item; key size 32, value size 8; .
    pub const ProxyDepositBase: Balance = deposit(1, 8);
    // Additional storage item size of 33 bytes.
    pub const ProxyDepositFactor: Balance = deposit(0, 33);
    pub const AnnouncementDepositBase: Balance = deposit(1, 8);
    pub const AnnouncementDepositFactor: Balance = deposit(0, 66);
}

/// The type used to represent the kinds of proxying allowed.
#[derive(
    Copy,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Encode,
    Decode,
    RuntimeDebug,
    MaxEncodedLen,
    scale_info::TypeInfo,
)]
pub enum ProxyType {
    Any,
    NonTransfer,
    Governance,
    Staking,
}
impl Default for ProxyType {
    fn default() -> Self {
        Self::Any
    }
}
impl InstanceFilter<RuntimeCall> for ProxyType {
    fn filter(&self, c: &RuntimeCall) -> bool {
        match self {
            ProxyType::Any => true,
            ProxyType::NonTransfer => !matches!(
                c,
                RuntimeCall::Balances(..) |
                    RuntimeCall::Assets(..) |
                    RuntimeCall::Nfts(..) |
                    RuntimeCall::Indices(pallet_indices::Call::transfer { .. })
            ),
            ProxyType::Governance => matches!(
                c,
                RuntimeCall::Democracy(..) |
                    RuntimeCall::Council(..) |
                    // RuntimeCall::Society(..) |
                    RuntimeCall::TechnicalCommittee(..) |
                    RuntimeCall::Elections(..) |
                    RuntimeCall::Treasury(..)
            ),
            ProxyType::Staking => matches!(c, RuntimeCall::Staking(..)),
        }
    }
    fn is_superset(&self, o: &Self) -> bool {
        match (self, o) {
            (x, y) if x == y => true,
            (ProxyType::Any, _) => true,
            (_, ProxyType::Any) => false,
            (ProxyType::NonTransfer, _) => true,
            _ => false,
        }
    }
}

impl pallet_proxy::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type Currency = Balances;
    type ProxyType = ProxyType;
    type ProxyDepositBase = ProxyDepositBase;
    type ProxyDepositFactor = ProxyDepositFactor;
    type MaxProxies = ConstU32<32>;
    type WeightInfo = pallet_proxy::weights::SubstrateWeight<Runtime>;
    type MaxPending = ConstU32<32>;
    type CallHasher = BlakeTwo256;
    type AnnouncementDepositBase = AnnouncementDepositBase;
    type AnnouncementDepositFactor = AnnouncementDepositFactor;
}

parameter_types! {
    pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) *
        RuntimeBlockWeights::get().max_block;
}

impl pallet_scheduler::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type PalletsOrigin = OriginCaller;
    type RuntimeCall = RuntimeCall;
    type MaximumWeight = MaximumSchedulerWeight;
    type ScheduleOrigin = EnsureRoot<AccountId>;
    #[cfg(feature = "runtime-benchmarks")]
    type MaxScheduledPerBlock = ConstU32<512>;
    #[cfg(not(feature = "runtime-benchmarks"))]
    type MaxScheduledPerBlock = ConstU32<50>;
    type WeightInfo = pallet_scheduler::weights::SubstrateWeight<Runtime>;
    type OriginPrivilegeCmp = EqualPrivilegeOnly;
    type Preimages = Preimage;
}

parameter_types! {
    pub const PreimageMaxSize: u32 = 4096 * 1024;
    pub const PreimageBaseDeposit: Balance = 1 * DOLLARS;
    // One cent: $10,000 / MB
    pub const PreimageByteDeposit: Balance = 1 * CENTS;
}

impl pallet_preimage::Config for Runtime {
    type WeightInfo = pallet_preimage::weights::SubstrateWeight<Runtime>;
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type ManagerOrigin = EnsureRoot<AccountId>;
    type BaseDeposit = PreimageBaseDeposit;
    type ByteDeposit = PreimageByteDeposit;
}

parameter_types! {
    // NOTE: Currently it is not possible to change the epoch duration after the chain has started.
    //       Attempting to do so will brick block production.
    pub const EpochDuration: u64 = EPOCH_DURATION_IN_SLOTS;
    pub const ExpectedBlockTime: Moment = MILLISECS_PER_BLOCK;
    pub const ReportLongevity: u64 =
        BondingDuration::get() as u64 * SessionsPerEra::get() as u64 * EpochDuration::get();
}

impl pallet_babe::Config for Runtime {
    type EpochDuration = EpochDuration;
    type ExpectedBlockTime = ExpectedBlockTime;
    type EpochChangeTrigger = pallet_babe::ExternalTrigger;
    type DisabledValidators = Session;
    type WeightInfo = ();
    type MaxAuthorities = MaxAuthorities;
    type KeyOwnerProof =
        <Historical as KeyOwnerProofSystem<(KeyTypeId, pallet_babe::AuthorityId)>>::Proof;
    type EquivocationReportSystem =
        pallet_babe::EquivocationReportSystem<Self, Offences, Historical, ReportLongevity>;
}

parameter_types! {
    pub const IndexDeposit: Balance = 1 * DOLLARS;
}

impl pallet_indices::Config for Runtime {
    type AccountIndex = AccountIndex;
    type Currency = Balances;
    type Deposit = IndexDeposit;
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_indices::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub const ExistentialDeposit: Balance = 0;
    // For weight estimation, we assume that the most locks on an individual account will be 50.
    // This number may need to be adjusted in the future if this assumption no longer holds true.
    pub const MaxLocks: u32 = 50;
    pub const MaxReserves: u32 = 50;
}

/// A reason for placing a hold on funds.
#[derive(
    Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Encode, Decode, MaxEncodedLen, Debug, TypeInfo,
)]
pub enum HoldReason {
    /// The NIS Pallet has reserved it for a non-fungible receipt.
    Nis,
    /// Used by the NFT Fractionalization Pallet.
    NftFractionalization,
}

impl pallet_balances::Config for Runtime {
    type MaxLocks = MaxLocks;
    type MaxReserves = MaxReserves;
    type ReserveIdentifier = [u8; 8];
    type Balance = Balance;
    type DustRemoval = ();
    type RuntimeEvent = RuntimeEvent;
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = frame_system::Pallet<Runtime>;
    type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
    type FreezeIdentifier = ();
    type MaxFreezes = ();
    type HoldIdentifier = HoldReason;
    type MaxHolds = ConstU32<2>;
}

parameter_types! {
    pub const TransactionByteFee: Balance = 10 * MILLICENTS;
    pub const OperationalFeeMultiplier: u8 = 5;
    pub const TargetBlockFullness: Perquintill = Perquintill::from_percent(25);
    pub AdjustmentVariable: Multiplier = Multiplier::saturating_from_rational(1, 100_000);
    pub MinimumMultiplier: Multiplier = Multiplier::saturating_from_rational(1, 1_000_000_000u128);
    pub MaximumMultiplier: Multiplier = Bounded::max_value();
}

impl pallet_transaction_payment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnChargeTransaction = CurrencyAdapter<Balances, DealWithFees<Runtime>>;
    type OperationalFeeMultiplier = OperationalFeeMultiplier;
    type WeightToFee = IdentityFee<Balance>;
    type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
    type FeeMultiplierUpdate = TargetedFeeAdjustment<
        Self,
        TargetBlockFullness,
        AdjustmentVariable,
        MinimumMultiplier,
        MaximumMultiplier,
    >;
}

impl pallet_asset_tx_payment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Fungibles = Assets;
    type OnChargeAssetTransaction = pallet_asset_tx_payment::FungiblesAdapter<
        pallet_assets::BalanceToAssetBalance<Balances, Runtime, ConvertInto>,
        CreditToBlockAuthor,
    >;
}

parameter_types! {
    pub const MinimumPeriod: Moment = SLOT_DURATION / 2;
}

impl pallet_timestamp::Config for Runtime {
    type Moment = Moment;
    type OnTimestampSet = Babe;
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = pallet_timestamp::weights::SubstrateWeight<Runtime>;
}

impl pallet_authorship::Config for Runtime {
    type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Babe>;
    type EventHandler = (Staking, ImOnline);
}

/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core data structures.
pub mod opaque {
    use super::*;
    pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

    /// Opaque block type.
    pub type Block = generic::Block<Header, UncheckedExtrinsic>;

    impl_opaque_keys! {
        pub struct SessionKeys {
            pub grandpa: Grandpa,
            pub babe: Babe,
            pub im_online: ImOnline,
            pub authority_discovery: AuthorityDiscovery,
        }
    }
}

impl pallet_session::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ValidatorId = <Self as frame_system::Config>::AccountId;
    type ValidatorIdOf = pallet_staking::StashOf<Self>;
    type ShouldEndSession = Babe;
    type NextSessionRotation = Babe;
    type SessionManager = pallet_session::historical::NoteHistoricalRoot<Self, Staking>;
    type SessionHandler = <opaque::SessionKeys as OpaqueKeys>::KeyTypeIdProviders;
    type Keys = opaque::SessionKeys;
    type WeightInfo = pallet_session::weights::SubstrateWeight<Runtime>;
}

impl pallet_session::historical::Config for Runtime {
    type FullIdentification = pallet_staking::Exposure<AccountId, Balance>;
    type FullIdentificationOf = pallet_staking::ExposureOf<Runtime>;
}

pallet_staking_reward_curve::build! {
    const REWARD_CURVE: PiecewiseLinear<'static> = curve!(
        min_inflation: 0_025_000,
        max_inflation: 0_100_000,
        ideal_stake: 0_500_000,
        falloff: 0_050_000,
        max_piece_count: 40,
        test_precision: 0_005_000,
    );
}

parameter_types! {
    pub const SessionsPerEra: sp_staking::SessionIndex = 6;
    pub const BondingDuration: sp_staking::EraIndex = 14;
    pub const SlashDeferDuration: sp_staking::EraIndex = 13;
    pub const RewardCurve: &'static PiecewiseLinear<'static> = &REWARD_CURVE;
    pub const MaxNominatorRewardedPerValidator: u32 = 256;
    pub const OffendingValidatorsThreshold: Perbill = Perbill::from_percent(17);
    pub OffchainRepeat: BlockNumber = 5;
    pub HistoryDepth: u32 = 84;
}

pub struct StakingBenchmarkingConfig;
impl pallet_staking::BenchmarkingConfig for StakingBenchmarkingConfig {
    type MaxNominators = ConstU32<1000>;
    type MaxValidators = ConstU32<1000>;
}

impl pallet_staking::Config for Runtime {
    type MaxNominations = MaxNominations;
    type Currency = Balances;
    type CurrencyBalance = Balance;
    type UnixTime = Timestamp;
    type CurrencyToVote = U128CurrencyToVote;
    type RewardRemainder = Treasury;
    type RuntimeEvent = RuntimeEvent;
    type Slash = Treasury; // send the slashed funds to the treasury.
    type Reward = (); // rewards are minted from the void
    type SessionsPerEra = SessionsPerEra;
    type BondingDuration = BondingDuration;
    type SlashDeferDuration = SlashDeferDuration;
    /// A super-majority of the council can cancel the slash.
    type AdminOrigin = EitherOfDiverse<
        EnsureRoot<AccountId>,
        pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 3, 4>,
    >;
    type SessionInterface = Self;
    type EraPayout = pallet_staking::ConvertCurve<RewardCurve>;
    type NextNewSession = Session;
    type MaxNominatorRewardedPerValidator = MaxNominatorRewardedPerValidator;
    type OffendingValidatorsThreshold = OffendingValidatorsThreshold;
    type ElectionProvider = ElectionProviderMultiPhase;
    type GenesisElectionProvider = onchain::OnChainExecution<OnChainSeqPhragmen>;
    type VoterList = VoterList;
    // This a placeholder, to be introduced in the next PR as an instance of bags-list
    type TargetList = pallet_staking::UseValidatorsMap<Self>;
    type MaxUnlockingChunks = ConstU32<32>;
    type HistoryDepth = HistoryDepth;
    type OnStakerSlash = NominationPools;
    type WeightInfo = pallet_staking::weights::SubstrateWeight<Runtime>;
    type BenchmarkingConfig = StakingBenchmarkingConfig;
    type MinimumPeriod = MinimumPeriod;
}

parameter_types! {
    // phase durations. 1/4 of the last session for each.
    pub const SignedPhase: u32 = EPOCH_DURATION_IN_BLOCKS / 4;
    pub const UnsignedPhase: u32 = EPOCH_DURATION_IN_BLOCKS / 4;

    // signed config
    pub const SignedRewardBase: Balance = 1 * DOLLARS;
    pub const SignedDepositBase: Balance = 1 * DOLLARS;
    pub const SignedDepositByte: Balance = 1 * CENTS;

    pub BetterUnsignedThreshold: Perbill = Perbill::from_rational(1u32, 10_000);

    // miner configs
    pub const MultiPhaseUnsignedPriority: TransactionPriority = StakingUnsignedPriority::get() - 1u64;
    pub MinerMaxWeight: Weight = RuntimeBlockWeights::get()
        .get(DispatchClass::Normal)
        .max_extrinsic.expect("Normal extrinsics have a weight limit configured; qed")
        .saturating_sub(BlockExecutionWeight::get());
    // Solution can occupy 90% of normal block size
    pub MinerMaxLength: u32 = Perbill::from_rational(9u32, 10) *
        *RuntimeBlockLength::get()
        .max
        .get(DispatchClass::Normal);
}

frame_election_provider_support::generate_solution_type!(
    #[compact]
    pub struct NposSolution16::<
        VoterIndex = u32,
        TargetIndex = u16,
        Accuracy = sp_runtime::PerU16,
        MaxVoters = MaxElectingVoters,
    >(16)
);

parameter_types! {
    pub MaxNominations: u32 = <NposSolution16 as frame_election_provider_support::NposSolution>::LIMIT as u32;
    pub MaxElectingVoters: u32 = 40_000;
    pub MaxElectableTargets: u16 = 10_000;
    // OnChain values are lower.
    pub MaxOnChainElectingVoters: u32 = 5000;
    pub MaxOnChainElectableTargets: u16 = 1250;
    // The maximum winners that can be elected by the Election pallet which is equivalent to the
    // maximum active validators the staking pallet can have.
    pub MaxActiveValidators: u32 = 1000;
}

/// The numbers configured here could always be more than the the maximum limits of staking pallet
/// to ensure election snapshot will not run out of memory. For now, we set them to smaller values
/// since the staking is bounded and the weight pipeline takes hours for this single pallet.
pub struct ElectionProviderBenchmarkConfig;
impl pallet_election_provider_multi_phase::BenchmarkingConfig for ElectionProviderBenchmarkConfig {
    const VOTERS: [u32; 2] = [1000, 2000];
    const TARGETS: [u32; 2] = [500, 1000];
    const ACTIVE_VOTERS: [u32; 2] = [500, 800];
    const DESIRED_TARGETS: [u32; 2] = [200, 400];
    const SNAPSHOT_MAXIMUM_VOTERS: u32 = 1000;
    const MINER_MAXIMUM_VOTERS: u32 = 1000;
    const MAXIMUM_TARGETS: u32 = 300;
}

/// Maximum number of iterations for balancing that will be executed in the embedded OCW
/// miner of election provider multi phase.
pub const MINER_MAX_ITERATIONS: u32 = 10;

/// A source of random balance for NposSolver, which is meant to be run by the OCW election miner.
pub struct OffchainRandomBalancing;
impl Get<Option<BalancingConfig>> for OffchainRandomBalancing {
    fn get() -> Option<BalancingConfig> {
        use sp_runtime::traits::TrailingZeroInput;
        let iterations = match MINER_MAX_ITERATIONS {
            0 => 0,
            max => {
                let seed = sp_io::offchain::random_seed();
                let random = <u32>::decode(&mut TrailingZeroInput::new(&seed))
                    .expect("input is padded with zeroes; qed") %
                    max.saturating_add(1);
                random as usize
            },
        };

        let config = BalancingConfig { iterations, tolerance: 0 };
        Some(config)
    }
}

pub struct OnChainSeqPhragmen;
impl onchain::Config for OnChainSeqPhragmen {
    type System = Runtime;
    type Solver = SequentialPhragmen<
        AccountId,
        pallet_election_provider_multi_phase::SolutionAccuracyOf<Runtime>,
    >;
    type DataProvider = <Runtime as pallet_election_provider_multi_phase::Config>::DataProvider;
    type WeightInfo = frame_election_provider_support::weights::SubstrateWeight<Runtime>;
    type MaxWinners = <Runtime as pallet_election_provider_multi_phase::Config>::MaxWinners;
    type VotersBound = MaxOnChainElectingVoters;
    type TargetsBound = MaxOnChainElectableTargets;
}

impl pallet_election_provider_multi_phase::MinerConfig for Runtime {
    type AccountId = AccountId;
    type MaxLength = MinerMaxLength;
    type MaxWeight = MinerMaxWeight;
    type Solution = NposSolution16;
    type MaxVotesPerVoter =
	<<Self as pallet_election_provider_multi_phase::Config>::DataProvider as ElectionDataProvider>::MaxVotesPerVoter;
    type MaxWinners = MaxActiveValidators;

    // The unsigned submissions have to respect the weight of the submit_unsigned call, thus their
    // weight estimate function is wired to this call's weight.
    fn solution_weight(v: u32, t: u32, a: u32, d: u32) -> Weight {
        <
			<Self as pallet_election_provider_multi_phase::Config>::WeightInfo
			as
			pallet_election_provider_multi_phase::WeightInfo
		>::submit_unsigned(v, t, a, d)
    }
}

type EnsureRootOrHalfCouncil = EitherOfDiverse<
    EnsureRoot<AccountId>,
    pallet_collective::EnsureProportionMoreThan<AccountId, CouncilCollective, 1, 2>,
>;

impl pallet_election_provider_multi_phase::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type EstimateCallFee = TransactionPayment;
    type SignedPhase = SignedPhase;
    type UnsignedPhase = UnsignedPhase;
    type BetterUnsignedThreshold = BetterUnsignedThreshold;
    type BetterSignedThreshold = ();
    type OffchainRepeat = OffchainRepeat;
    type MinerTxPriority = MultiPhaseUnsignedPriority;
    type MinerConfig = Self;
    type SignedMaxSubmissions = ConstU32<10>;
    type SignedRewardBase = SignedRewardBase;
    type SignedDepositBase = SignedDepositBase;
    type SignedDepositByte = SignedDepositByte;
    type SignedMaxRefunds = ConstU32<3>;
    type SignedDepositWeight = ();
    type SignedMaxWeight = MinerMaxWeight;
    type SlashHandler = (); // burn slashes
    type RewardHandler = (); // nothing to do upon rewards
    type DataProvider = Staking;
    type Fallback = onchain::OnChainExecution<OnChainSeqPhragmen>;
    type GovernanceFallback = onchain::OnChainExecution<OnChainSeqPhragmen>;
    type Solver = SequentialPhragmen<AccountId, SolutionAccuracyOf<Self>, OffchainRandomBalancing>;
    type ForceOrigin = EnsureRootOrHalfCouncil;
    type MaxElectableTargets = MaxElectableTargets;
    type MaxWinners = MaxActiveValidators;
    type MaxElectingVoters = MaxElectingVoters;
    type BenchmarkingConfig = ElectionProviderBenchmarkConfig;
    type WeightInfo = pallet_election_provider_multi_phase::weights::SubstrateWeight<Self>;
}

parameter_types! {
    pub const BagThresholds: &'static [u64] = &voter_bags::THRESHOLDS;
}

type VoterBagsListInstance = pallet_bags_list::Instance1;
impl pallet_bags_list::Config<VoterBagsListInstance> for Runtime {
    type RuntimeEvent = RuntimeEvent;
    /// The voter bags-list is loosely kept up to date, and the real source of truth for the score
    /// of each node is the staking pallet.
    type ScoreProvider = Staking;
    type BagThresholds = BagThresholds;
    type Score = VoteWeight;
    type WeightInfo = pallet_bags_list::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub const PostUnbondPoolsWindow: u32 = 4;
    pub const NominationPoolsPalletId: PalletId = PalletId(*b"py/nopls");
    pub const MaxPointsToBalance: u8 = 10;
}

use sp_runtime::traits::Convert;

pub struct BalanceToU256;
impl Convert<Balance, sp_core::U256> for BalanceToU256 {
    fn convert(balance: Balance) -> sp_core::U256 {
        sp_core::U256::from(balance)
    }
}
pub struct U256ToBalance;
impl Convert<sp_core::U256, Balance> for U256ToBalance {
    fn convert(n: sp_core::U256) -> Balance {
        n.try_into().unwrap_or(Balance::max_value())
    }
}

impl pallet_nomination_pools::Config for Runtime {
    type WeightInfo = ();
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type RewardCounter = FixedU128;
    type BalanceToU256 = BalanceToU256;
    type U256ToBalance = U256ToBalance;
    type Staking = Staking;
    type PostUnbondingPoolsWindow = PostUnbondPoolsWindow;
    type MaxMetadataLen = ConstU32<256>;
    type MaxUnbonding = ConstU32<8>;
    type PalletId = NominationPoolsPalletId;
    type MaxPointsToBalance = MaxPointsToBalance;
}

parameter_types! {
    pub const LaunchPeriod: BlockNumber = 28 * 24 * 60 * MINUTES;
    pub const VotingPeriod: BlockNumber = 28 * 24 * 60 * MINUTES;
    pub const FastTrackVotingPeriod: BlockNumber = 3 * 24 * 60 * MINUTES;
    pub const MinimumDeposit: Balance = 10000 * DBCS;
    pub const EnactmentPeriod: BlockNumber = 30 * 24 * 60 * MINUTES;
    pub const CooloffPeriod: BlockNumber = 28 * 24 * 60 * MINUTES;
    pub const MaxProposals: u32 = 100;
}

impl pallet_democracy::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type EnactmentPeriod = EnactmentPeriod;
    type LaunchPeriod = LaunchPeriod;
    type VotingPeriod = VotingPeriod;
    type VoteLockingPeriod = EnactmentPeriod; // Same as EnactmentPeriod
    type MinimumDeposit = MinimumDeposit;
    /// A straight majority of the council can decide what their next motion is.
    type ExternalOrigin =
        pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 1, 2>;
    /// A super-majority can have the next scheduled referendum be a straight majority-carries vote.
    type ExternalMajorityOrigin =
        pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 3, 4>;
    /// A unanimous council can have the next scheduled referendum be a straight default-carries
    /// (NTB) vote.
    type ExternalDefaultOrigin =
        pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 1, 1>;
    type SubmitOrigin = EnsureSigned<AccountId>;
    /// Two thirds of the technical committee can have an ExternalMajority/ExternalDefault vote
    /// be tabled immediately and with a shorter voting/enactment period.
    type FastTrackOrigin =
        pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 2, 3>;
    type InstantOrigin =
        pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 1, 1>;
    type InstantAllowed = frame_support::traits::ConstBool<true>;
    type FastTrackVotingPeriod = FastTrackVotingPeriod;
    // To cancel a proposal which has been passed, 2/3 of the council must agree to it.
    type CancellationOrigin =
        pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 2, 3>;
    // To cancel a proposal before it has been passed, the technical committee must be unanimous or
    // Root must agree.
    type CancelProposalOrigin = EitherOfDiverse<
        EnsureRoot<AccountId>,
        pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 1, 1>,
    >;
    type BlacklistOrigin = EnsureRoot<AccountId>;
    // Any single technical committee member may veto a coming council proposal, however they can
    // only do it once and it lasts only for the cool-off period.
    type VetoOrigin = pallet_collective::EnsureMember<AccountId, TechnicalCollective>;
    type CooloffPeriod = CooloffPeriod;
    type Slash = Treasury;
    type Scheduler = Scheduler;
    type PalletsOrigin = OriginCaller;
    type MaxVotes = ConstU32<100>;
    type WeightInfo = pallet_democracy::weights::SubstrateWeight<Runtime>;
    type MaxProposals = MaxProposals;
    type Preimages = Preimage;
    type MaxDeposits = ConstU32<100>;
    type MaxBlacklisted = ConstU32<100>;
}

parameter_types! {
    pub const CouncilMotionDuration: BlockNumber = 5 * DAYS;
    pub const CouncilMaxProposals: u32 = 100;
    pub const CouncilMaxMembers: u32 = 100;
}

type CouncilCollective = pallet_collective::Instance1;
impl pallet_collective::Config<CouncilCollective> for Runtime {
    type RuntimeOrigin = RuntimeOrigin;
    type Proposal = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type MotionDuration = CouncilMotionDuration;
    type MaxProposals = CouncilMaxProposals;
    type MaxMembers = CouncilMaxMembers;
    type DefaultVote = pallet_collective::PrimeDefaultVote;
    type WeightInfo = pallet_collective::weights::SubstrateWeight<Runtime>;
    type SetMembersOrigin = EnsureRoot<Self::AccountId>;
    type MaxProposalWeight = MaxCollectivesProposalWeight;
}

parameter_types! {
    pub const CandidacyBond: Balance = 10000 * DBCS;
    // 1 storage item created, key size is 32 bytes, value size is 16+16.
    pub const VotingBondBase: Balance = deposit(1, 64);
    // additional data per vote is 32 bytes (account id).
    pub const VotingBondFactor: Balance = deposit(0, 32);
    pub const TermDuration: BlockNumber = 120 * DAYS;
    pub const DesiredMembers: u32 = 21;
    pub const DesiredRunnersUp: u32 = 7;
    pub const MaxVotesPerVoter: u32 = 16;
    pub const MaxVoters: u32 = 10 * 1000;
    pub const MaxCandidates: u32 = 1000;
    pub const ElectionsPhragmenPalletId: LockIdentifier = *b"phrelect";
}

// Make sure that there are no more than `MaxMembers` members elected via elections-phragmen.
const_assert!(DesiredMembers::get() <= CouncilMaxMembers::get());

impl pallet_elections_phragmen::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type PalletId = ElectionsPhragmenPalletId;
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
    type MaxVoters = MaxVoters;
    type MaxVotesPerVoter = MaxVotesPerVoter;
    type MaxCandidates = MaxCandidates;
    type WeightInfo = pallet_elections_phragmen::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub const TechnicalMotionDuration: BlockNumber = 10 * MINUTES;
    pub const TechnicalMaxProposals: u32 = 100;
    pub const TechnicalMaxMembers: u32 = 100;
}

type TechnicalCollective = pallet_collective::Instance2;
impl pallet_collective::Config<TechnicalCollective> for Runtime {
    type RuntimeOrigin = RuntimeOrigin;
    type Proposal = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type MotionDuration = TechnicalMotionDuration;
    type MaxProposals = TechnicalMaxProposals;
    type MaxMembers = TechnicalMaxMembers;
    type DefaultVote = pallet_collective::PrimeDefaultVote;
    type WeightInfo = pallet_collective::weights::SubstrateWeight<Runtime>;
    type SetMembersOrigin = EnsureRoot<Self::AccountId>;
    type MaxProposalWeight = MaxCollectivesProposalWeight;
}

parameter_types! {
    pub const ProposalBond: Permill = Permill::from_percent(5);
    pub const ProposalBondMinimum: Balance = 1 * DOLLARS;
    pub const SpendPeriod: BlockNumber = 12 * DAYS;
    pub Burn: Permill = PerThing::from_rational(5u32, 1000u32);
    pub const TipCountdown: BlockNumber = 1 * DAYS;
    pub const TipFindersFee: Percent = Percent::from_percent(20);
    pub const TipReportDepositBase: Balance = 1 * DOLLARS;
    pub const DataDepositPerByte: Balance = 1 * CENTS;
    pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
    pub const MaximumReasonLength: u32 = 300;
    pub const MaxApprovals: u32 = 100;
    pub const MaxBalance: Balance = Balance::max_value();
}

impl pallet_treasury::Config for Runtime {
    type PalletId = TreasuryPalletId;
    type Currency = Balances;
    type ApproveOrigin = EitherOfDiverse<
        EnsureRoot<AccountId>,
        pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 3, 5>,
    >;
    type RejectOrigin = EitherOfDiverse<
        EnsureRoot<AccountId>,
        pallet_collective::EnsureProportionMoreThan<AccountId, CouncilCollective, 1, 2>,
    >;
    type RuntimeEvent = RuntimeEvent;
    type OnSlash = ();
    type ProposalBond = ProposalBond;
    type ProposalBondMinimum = ProposalBondMinimum;
    type ProposalBondMaximum = ();
    type SpendPeriod = SpendPeriod;
    type Burn = Burn;
    type BurnDestination = ();
    type SpendFunds = Bounties;
    type WeightInfo = pallet_treasury::weights::SubstrateWeight<Runtime>;
    type MaxApprovals = MaxApprovals;
    type SpendOrigin = EnsureWithSuccess<EnsureRoot<AccountId>, AccountId, MaxBalance>;
}

parameter_types! {
    pub const BountyCuratorDeposit: Permill = Permill::from_percent(50);
    pub const BountyValueMinimum: Balance = 5 * DOLLARS;
    pub const BountyDepositBase: Balance = 1 * DOLLARS;
    pub const CuratorDepositMultiplier: Permill = Permill::from_percent(50);
    pub const CuratorDepositMin: Balance = 1 * DOLLARS;
    pub const CuratorDepositMax: Balance = 100 * DOLLARS;
    pub const BountyDepositPayoutDelay: BlockNumber = 1 * DAYS;
    pub const BountyUpdatePeriod: BlockNumber = 14 * DAYS;
}

impl pallet_bounties::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type BountyDepositBase = BountyDepositBase;
    type BountyDepositPayoutDelay = BountyDepositPayoutDelay;
    type BountyUpdatePeriod = BountyUpdatePeriod;
    type CuratorDepositMultiplier = CuratorDepositMultiplier;
    type CuratorDepositMin = CuratorDepositMin;
    type CuratorDepositMax = CuratorDepositMax;
    type BountyValueMinimum = BountyValueMinimum;
    type DataDepositPerByte = DataDepositPerByte;
    type MaximumReasonLength = MaximumReasonLength;
    type WeightInfo = pallet_bounties::weights::SubstrateWeight<Runtime>;
    type ChildBountyManager = ChildBounties;
}

parameter_types! {
    pub const ChildBountyValueMinimum: Balance = 1 * DOLLARS;
}

impl pallet_child_bounties::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MaxActiveChildBountyCount = ConstU32<5>;
    type ChildBountyValueMinimum = ChildBountyValueMinimum;
    type WeightInfo = pallet_child_bounties::weights::SubstrateWeight<Runtime>;
}

impl pallet_tips::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type DataDepositPerByte = DataDepositPerByte;
    type MaximumReasonLength = MaximumReasonLength;
    type Tippers = Elections;
    type TipCountdown = TipCountdown;
    type TipFindersFee = TipFindersFee;
    type TipReportDepositBase = TipReportDepositBase;
    type WeightInfo = pallet_tips::weights::SubstrateWeight<Runtime>;
}

impl pallet_sudo::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type WeightInfo = pallet_sudo::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub const ImOnlineUnsignedPriority: TransactionPriority = TransactionPriority::max_value();
    /// We prioritize im-online heartbeats over election solution submission.
    pub const StakingUnsignedPriority: TransactionPriority = TransactionPriority::max_value() / 2;
    pub const MaxAuthorities: u32 = 100;
    pub const MaxKeys: u32 = 10_000;
    pub const MaxPeerInHeartbeats: u32 = 10_000;
    pub const MaxPeerDataEncodingSize: u32 = 1_000;
}

impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for Runtime
where
    RuntimeCall: From<LocalCall>,
{
    fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
        call: RuntimeCall,
        public: <Signature as traits::Verify>::Signer,
        account: AccountId,
        nonce: Index,
    ) -> Option<(RuntimeCall, <UncheckedExtrinsic as traits::Extrinsic>::SignaturePayload)> {
        let tip = 0;
        // take the biggest period possible.
        let period =
            BlockHashCount::get().checked_next_power_of_two().map(|c| c / 2).unwrap_or(2) as u64;
        let current_block = System::block_number()
            .saturated_into::<u64>()
            // The `System::block_number` is initialized with `n+1`,
            // so the actual block number is `n`.
            .saturating_sub(1);
        let era = Era::mortal(period, current_block);
        let extra = (
            frame_system::CheckNonZeroSender::<Runtime>::new(),
            frame_system::CheckSpecVersion::<Runtime>::new(),
            frame_system::CheckTxVersion::<Runtime>::new(),
            frame_system::CheckGenesis::<Runtime>::new(),
            frame_system::CheckEra::<Runtime>::from(era),
            frame_system::CheckNonce::<Runtime>::from(nonce),
            frame_system::CheckWeight::<Runtime>::new(),
            pallet_asset_tx_payment::ChargeAssetTxPayment::<Runtime>::from(tip, None),
        );
        let raw_payload = SignedPayload::new(call, extra)
            .map_err(|e| {
                log::warn!(target: TARGET, "Unable to create signed payload: {:?}", e);
            })
            .ok()?;
        let signature = raw_payload.using_encoded(|payload| C::sign(payload, public))?;
        let address = Indices::unlookup(account);
        let (call, extra, _) = raw_payload.deconstruct();
        Some((call, (address, signature, extra)))
    }
}

impl frame_system::offchain::SigningTypes for Runtime {
    type Public = <Signature as traits::Verify>::Signer;
    type Signature = Signature;
}

impl<C> frame_system::offchain::SendTransactionTypes<C> for Runtime
where
    RuntimeCall: From<C>,
{
    type Extrinsic = UncheckedExtrinsic;
    type OverarchingCall = RuntimeCall;
}

impl pallet_im_online::Config for Runtime {
    type AuthorityId = ImOnlineId;
    type RuntimeEvent = RuntimeEvent;
    type NextSessionRotation = Babe;
    type ValidatorSet = Historical;
    type ReportUnresponsiveness = Offences;
    type UnsignedPriority = ImOnlineUnsignedPriority;
    type WeightInfo = pallet_im_online::weights::SubstrateWeight<Runtime>;
    type MaxKeys = MaxKeys;
    type MaxPeerInHeartbeats = MaxPeerInHeartbeats;
    type MaxPeerDataEncodingSize = MaxPeerDataEncodingSize;
}

impl pallet_offences::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type IdentificationTuple = pallet_session::historical::IdentificationTuple<Self>;
    type OnOffenceHandler = Staking;
}

impl pallet_authority_discovery::Config for Runtime {
    type MaxAuthorities = MaxAuthorities;
}

parameter_types! {
    pub const MaxSetIdSessionEntries: u32 = BondingDuration::get() * SessionsPerEra::get();
}

impl pallet_grandpa::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type MaxAuthorities = MaxAuthorities;
    type MaxSetIdSessionEntries = MaxSetIdSessionEntries;
    type KeyOwnerProof = <Historical as KeyOwnerProofSystem<(KeyTypeId, GrandpaId)>>::Proof;
    type EquivocationReportSystem =
        pallet_grandpa::EquivocationReportSystem<Self, Offences, Historical, ReportLongevity>;
}

parameter_types! {
    pub const BasicDeposit: Balance = 10 * DOLLARS;       // 258 bytes on-chain
    pub const FieldDeposit: Balance = 250 * CENTS;        // 66 bytes on-chain
    pub const SubAccountDeposit: Balance = 2 * DOLLARS;   // 53 bytes on-chain
    pub const MaxSubAccounts: u32 = 100;
    pub const MaxAdditionalFields: u32 = 100;
    pub const MaxRegistrars: u32 = 20;
}

impl pallet_identity::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type BasicDeposit = BasicDeposit;
    type FieldDeposit = FieldDeposit;
    type SubAccountDeposit = SubAccountDeposit;
    type MaxSubAccounts = MaxSubAccounts;
    type MaxAdditionalFields = MaxAdditionalFields;
    type MaxRegistrars = MaxRegistrars;
    type Slashed = Treasury;
    type ForceOrigin = EnsureRootOrHalfCouncil;
    type RegistrarOrigin = EnsureRootOrHalfCouncil;
    type WeightInfo = pallet_identity::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub const ConfigDepositBase: Balance = 5 * DOLLARS;
    pub const FriendDepositFactor: Balance = 50 * CENTS;
    pub const MaxFriends: u16 = 9;
    pub const RecoveryDeposit: Balance = 5 * DOLLARS;
}

impl pallet_recovery::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_recovery::weights::SubstrateWeight<Runtime>;
    type RuntimeCall = RuntimeCall;
    type Currency = Balances;
    type ConfigDepositBase = ConfigDepositBase;
    type FriendDepositFactor = FriendDepositFactor;
    type MaxFriends = MaxFriends;
    type RecoveryDeposit = RecoveryDeposit;
}

parameter_types! {
    pub const AssetDeposit: Balance = 100 * DOLLARS;
    pub const ApprovalDeposit: Balance = 1 * DOLLARS;
    pub const StringLimit: u32 = 50;
    pub const MetadataDepositBase: Balance = 10 * DOLLARS;
    pub const MetadataDepositPerByte: Balance = 1 * DOLLARS;
    pub const MinLockAmount: Balance = 100 * ONE_DLC;
    pub const MaxLockDuration: BlockNumber  = 1800 * DAYS;
}

impl pallet_assets::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Balance = u128;
    type AssetId = u32;
    type AssetIdParameter = Compact<u32>;
    type Currency = Balances;
    type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
    type ForceOrigin = EnsureRoot<AccountId>;
    type AssetDeposit = AssetDeposit;
    type AssetAccountDeposit = ConstU128<DOLLARS>;
    type MetadataDepositBase = MetadataDepositBase;
    type MetadataDepositPerByte = MetadataDepositPerByte;
    type ApprovalDeposit = ApprovalDeposit;
    type StringLimit = StringLimit;
    type Freezer = ();
    type Extra = ();
    type CallbackHandle = ();
    type WeightInfo = pallet_assets::weights::SubstrateWeight<Runtime>;
    type RemoveItemsLimit = ConstU32<1000>;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = ();

    type MinLockAmount = MinLockAmount;

    type MaxLockDuration = MaxLockDuration;
}

parameter_types! {
    pub const CollectionDeposit: Balance = 100 * DOLLARS;
    pub const ItemDeposit: Balance = 1 * DOLLARS;
    pub const KeyLimit: u32 = 32;
    pub const ValueLimit: u32 = 256;
    pub const ApprovalsLimit: u32 = 20;
    pub const ItemAttributesApprovalsLimit: u32 = 20;
    pub const MaxTips: u32 = 10;
    pub const MaxDeadlineDuration: BlockNumber = 12 * 30 * DAYS;
}

parameter_types! {
    pub Features: PalletFeatures = PalletFeatures::all_enabled();
    pub const MaxAttributesPerCall: u32 = 10;
}

impl pallet_nfts::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type CollectionId = u32;
    type ItemId = u32;
    type Currency = Balances;
    type ForceOrigin = frame_system::EnsureRoot<AccountId>;
    type CollectionDeposit = CollectionDeposit;
    type ItemDeposit = ItemDeposit;
    type MetadataDepositBase = MetadataDepositBase;
    type AttributeDepositBase = MetadataDepositBase;
    type DepositPerByte = MetadataDepositPerByte;
    type StringLimit = StringLimit;
    type KeyLimit = KeyLimit;
    type ValueLimit = ValueLimit;
    type ApprovalsLimit = ApprovalsLimit;
    type ItemAttributesApprovalsLimit = ItemAttributesApprovalsLimit;
    type MaxTips = MaxTips;
    type MaxDeadlineDuration = MaxDeadlineDuration;
    type MaxAttributesPerCall = MaxAttributesPerCall;
    type Features = Features;
    type OffchainSignature = Signature;
    type OffchainPublic = <Signature as traits::Verify>::Signer;
    type WeightInfo = pallet_nfts::weights::SubstrateWeight<Runtime>;
    #[cfg(feature = "runtime-benchmarks")]
    type Helper = ();
    type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
    type Locker = ();
}

impl generic_func::Config for Runtime {
    type Currency = Balances;
    type RuntimeEvent = RuntimeEvent;
    type RandomnessSource = RandomnessCollectiveFlip;
    type FixedTxFee = Treasury;
    type Slash = Treasury;
}

parameter_types! {
    // 首要投票人：5000 USD 或 60万 DBC 取价值较小
    pub const PrimerReward: (u64, Balance) = (5_000_000_000u64, 600_000 * DBCS);
    // 排名第二的议会成员：2000 USD 或 20万 DBC 取价值较小
    pub const SecondReward: (u64, Balance) = (2_000_000_000u64, 200_000 * DBCS);
    // 排名第三的议会成员：2000 USD 或 20万 DBC 取价值较小
    pub const ThirdReward: (u64, Balance) = (2_000_000_000u64, 200_000 * DBCS);
    // 发放周期
    pub const RewardFrequency: BlockNumber = 30 * DAYS;
}

impl council_reward::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type DbcPrice = DBCPriceOCW;
    type Currency = Balances;
    type RewardFrequency = RewardFrequency;
    type PrimerReward = PrimerReward;
    type SecondReward = SecondReward;
    type ThirdReward = ThirdReward;
}

impl dbc_price_ocw::Config for Runtime {
    type Currency = Balances;
    type RuntimeEvent = RuntimeEvent;
    type RandomnessSource = RandomnessCollectiveFlip;
}

impl dlc_price_ocw::Config for Runtime {
    type Currency = Balances;
    type RuntimeEvent = RuntimeEvent;
    type RandomnessSource = RandomnessCollectiveFlip;
}

impl online_profile::Config for Runtime {
    type Currency = Balances;
    type RuntimeEvent = RuntimeEvent;
    type BondingDuration = BondingDuration;
    type DbcPrice = DBCPriceOCW;
    type ManageCommittee = Committee;
    type Slash = Treasury;
    type CancelSlashOrigin =
        pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 1, 5>;
    type SlashAndReward = GenericFunc;
}

impl committee::Config for Runtime {
    type Currency = Balances;
    type RuntimeEvent = RuntimeEvent;
    // type WeightInfo = committee::weights::SubstrateWeight<Runtime>;
}

impl online_committee::Config for Runtime {
    type Currency = Balances;
    type RuntimeEvent = RuntimeEvent;
    type OCOps = OnlineProfile;
    type ManageCommittee = Committee;
    type CancelSlashOrigin =
        pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 1, 5>;
    type SlashAndReward = GenericFunc;
}

impl rent_machine::Config for Runtime {
    type Currency = Balances;
    type RuntimeEvent = RuntimeEvent;
    type RTOps = OnlineProfile;
    type DbcPrice = DBCPriceOCW;
}

impl maintain_committee::Config for Runtime {
    type Currency = Balances;
    type RuntimeEvent = RuntimeEvent;
    type ManageCommittee = Committee;
    type MTOps = OnlineProfile;
    type Slash = Treasury;
    type CancelSlashOrigin =
        pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 1, 5>;
    type SlashAndReward = GenericFunc;
}

impl terminating_rental::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type Slash = Treasury;
    type ManageCommittee = Committee;
    type DbcPrice = DBCPriceOCW;
    type SlashAndReward = GenericFunc;
}

impl simple_rpc::Config for Runtime {
    type Currency = Balances;
    type OPRpcQuery = OnlineProfile;
}

impl pallet_evm_chain_id::Config for Runtime {}

/// Current approximation of the gas/s consumption considering
/// EVM execution over compiled WASM (on 4.4Ghz CPU).
/// Given the 500ms Weight, from which 75% only are used for transactions,
/// the total EVM execution gas limit is: GAS_PER_SECOND * 0.500 * 0.75 ~= 15_000_000.
pub const GAS_PER_SECOND: u64 = 40_000_000;

// Weight correspond to 1 picosecond
pub const WEIGHT_PER_SECOND: u64 = 1_000_000_000_000;

/// Approximate ratio of the amount of Weight per Gas.
/// u64 works for approximations because Weight is a very small unit compared to gas.
pub const WEIGHT_PER_GAS: u64 = WEIGHT_PER_SECOND / GAS_PER_SECOND;

const BLOCK_GAS_LIMIT: u64 = 75_000_000;
const MAX_POV_SIZE: u64 = 5 * 1024 * 1024;

/// We allow for 2000ms of compute with a 6 second average block time.
pub const WEIGHT_MILLISECS_PER_BLOCK: u64 = 2000;

parameter_types! {
    pub BlockGasLimit: U256
        = U256::from(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT.ref_time() / WEIGHT_PER_GAS);
    pub const GasLimitPovSizeRatio: u64 = BLOCK_GAS_LIMIT.saturating_div(MAX_POV_SIZE);
    pub PrecompilesValue: DBCPrecompiles<Runtime> = DBCPrecompiles::<_>::new();
    pub WeightPerGas: Weight = Weight::from_parts(weight_per_gas(BLOCK_GAS_LIMIT, NORMAL_DISPATCH_RATIO, WEIGHT_MILLISECS_PER_BLOCK), 0);
}

type CurrencyAccountId<T> = <T as frame_system::Config>::AccountId;

type BalanceFor<T> =
    <<T as pallet_evm::Config>::Currency as CurrencyT<CurrencyAccountId<T>>>::Balance;

type PositiveImbalanceFor<T> =
    <<T as pallet_evm::Config>::Currency as CurrencyT<CurrencyAccountId<T>>>::PositiveImbalance;

type NegativeImbalanceFor<T> =
    <<T as pallet_evm::Config>::Currency as CurrencyT<CurrencyAccountId<T>>>::NegativeImbalance;

pub struct OnChargeEVMTransaction<OU>(sp_std::marker::PhantomData<OU>);
impl<T, OU> OnChargeEVMTransactionT<T> for OnChargeEVMTransaction<OU>
where
    T: pallet_evm::Config,
    PositiveImbalanceFor<T>: Imbalance<BalanceFor<T>, Opposite = NegativeImbalanceFor<T>>,
    NegativeImbalanceFor<T>: Imbalance<BalanceFor<T>, Opposite = PositiveImbalanceFor<T>>,
    OU: OnUnbalanced<NegativeImbalanceFor<T>>,
    U256: UniqueSaturatedInto<BalanceFor<T>>,
{
    type LiquidityInfo = Option<NegativeImbalanceFor<T>>;

    fn withdraw_fee(who: &H160, fee: U256) -> Result<Self::LiquidityInfo, pallet_evm::Error<T>> {
        EVMCurrencyAdapter::<<T as pallet_evm::Config>::Currency, ()>::withdraw_fee(who, fee)
    }

    fn correct_and_deposit_fee(
        who: &H160,
        corrected_fee: U256,
        base_fee: U256,
        already_withdrawn: Self::LiquidityInfo,
    ) -> Self::LiquidityInfo {
        <EVMCurrencyAdapter<<T as pallet_evm::Config>::Currency, OU> as OnChargeEVMTransactionT<
            T,
        >>::correct_and_deposit_fee(who, corrected_fee, base_fee, already_withdrawn)
    }

    fn pay_priority_fee(tip: Self::LiquidityInfo) {
        if let Some(tip) = tip {
            OU::on_unbalanced(tip);
        }
    }
}

static DBC_EVM_CONFIG: EvmConfig =
    EvmConfig { create_contract_limit: Some(0x12000), ..EvmConfig::shanghai() };

impl pallet_evm::Config for Runtime {
    type FeeCalculator = BaseFee;
    type GasWeightMapping = pallet_evm::FixedGasWeightMapping<Self>;
    type WeightPerGas = WeightPerGas;
    type BlockHashMapping = pallet_ethereum::EthereumBlockHashMapping<Self>;
    type CallOrigin = EnsureAddressRoot<AccountId>;
    type WithdrawOrigin = EnsureAddressNever<AccountId>;
    type AddressMapping = HashedAddressMapping<BlakeTwo256>;
    type Currency = Balances;
    type RuntimeEvent = RuntimeEvent;
    type PrecompilesType = DBCPrecompiles<Self>;
    type PrecompilesValue = PrecompilesValue;
    type ChainId = EVMChainId;
    type BlockGasLimit = BlockGasLimit;
    type Runner = pallet_evm::runner::stack::Runner<Self>;
    type OnChargeTransaction = OnChargeEVMTransaction<DealWithFees<Runtime>>;
    type OnCreate = ();
    type FindAuthor = ();
    type GasLimitPovSizeRatio = GasLimitPovSizeRatio;
    type Timestamp = Timestamp;
    type WeightInfo = pallet_evm::weights::SubstrateWeight<Self>;

    fn config() -> &'static EvmConfig {
        &DBC_EVM_CONFIG
    }
}

parameter_types! {
    pub const PostBlockAndTxnHashes: PostLogContent = PostLogContent::BlockAndTxnHashes;
}

impl pallet_ethereum::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type StateRoot = pallet_ethereum::IntermediateStateRoot<Self>;
    type PostLogContent = PostBlockAndTxnHashes;
    type ExtraDataLength = ConstU32<30>;
}

impl eth_precompile_whitelist::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WhitelistLimit = ConstU32<10>;
}

parameter_types! {
    pub DefaultBaseFeePerGas: U256 = U256::from(10_000_000_000u128);
    // No gas price adjustment for now. default is 125_000 (12.5%)
    pub DefaultElasticity: Permill = Permill::from_parts(0);
}

pub struct BaseFeeThreshold;
impl pallet_base_fee::BaseFeeThreshold for BaseFeeThreshold {
    fn lower() -> Permill {
        Permill::zero()
    }
    fn ideal() -> Permill {
        Permill::from_parts(500_000)
    }
    fn upper() -> Permill {
        Permill::from_parts(1_000_000)
    }
}

impl pallet_base_fee::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Threshold = BaseFeeThreshold;
    type DefaultBaseFeePerGas = DefaultBaseFeePerGas;
    type DefaultElasticity = DefaultElasticity;
}
const ALLIANCE_MOTION_DURATION_IN_BLOCKS: BlockNumber = 5 * DAYS;

parameter_types! {
    // pub const MaxFellows: u32 = AllianceMaxMembers::get();
    pub const MaxAllies: u32 = 100;
    pub const AllyDeposit: Balance = 10 * DOLLARS;
    pub const RetirementPeriod: BlockNumber = ALLIANCE_MOTION_DURATION_IN_BLOCKS + (1 * DAYS);
}

construct_runtime!(
    pub enum Runtime where
        Block = Block,
        NodeBlock = dbc_primitives::Block,
        UncheckedExtrinsic = UncheckedExtrinsic
    {
        System: frame_system = 0,
        Utility: pallet_utility = 1,
        Timestamp: pallet_timestamp = 2,
        Babe: pallet_babe = 3,
        Grandpa: pallet_grandpa = 4,
        // Authorship must be before session in order to note author in the correct session and era
        // for im-online and staking.
        Authorship: pallet_authorship = 5,
        Indices: pallet_indices = 6,
        Balances: pallet_balances = 7,
        TransactionPayment: pallet_transaction_payment = 8,
        AssetTxPayment: pallet_asset_tx_payment = 9,
        Session: pallet_session = 10,
        Historical: pallet_session_historical::{Pallet} = 11,
        Democracy: pallet_democracy = 12,
        Council: pallet_collective::<Instance1> = 13,
        TechnicalCommittee: pallet_collective::<Instance2> = 14,
        Elections: pallet_elections_phragmen = 15,
        ElectionProviderMultiPhase: pallet_election_provider_multi_phase = 16,
        Treasury: pallet_treasury = 18,
        ImOnline: pallet_im_online = 19,
        AuthorityDiscovery: pallet_authority_discovery = 20,
        Offences: pallet_offences = 21,
        RandomnessCollectiveFlip: pallet_insecure_randomness_collective_flip = 22,
        Identity: pallet_identity = 23,
        Recovery: pallet_recovery = 24,
        Preimage: pallet_preimage = 25,
        Proxy: pallet_proxy = 26,
        Multisig: pallet_multisig = 27,
        Bounties: pallet_bounties = 28,
        ChildBounties: pallet_child_bounties = 29,
        Tips: pallet_tips = 30,
        VoterList: pallet_bags_list::<Instance1> = 31,
        NominationPools: pallet_nomination_pools = 32,
        Sudo: pallet_sudo = 33,

        // Evm
        Ethereum: pallet_ethereum = 50,
        EVM: pallet_evm = 51,
        EVMChainId: pallet_evm_chain_id = 52,
        BaseFee: pallet_base_fee = 53,

        // DBC pallets
        Staking: pallet_staking = 100,
        Assets: pallet_assets = 101,
        Nfts: pallet_nfts = 102,
        Scheduler: pallet_scheduler = 103,
        GenericFunc: generic_func = 104,
        CouncilReward: council_reward = 105,
        DBCPriceOCW: dbc_price_ocw = 106,
        OnlineProfile: online_profile = 107,
        Committee: committee = 108,
        SimpleRpc: simple_rpc = 109,
        OnlineCommittee: online_committee = 110,
        RentMachine: rent_machine = 111,
        MaintainCommittee: maintain_committee = 112,
        TerminatingRental: terminating_rental = 113,
        EthPrecompileWhitelist: eth_precompile_whitelist = 114,
        DLCPriceOCW: dlc_price_ocw = 115,

    }
);

/// The address format for describing accounts.
pub type Address = sp_runtime::MultiAddress<AccountId, AccountIndex>;
/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;
/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;
/// The SignedExtension to the basic transaction logic.
///
/// When you change this, you **MUST** modify [`sign`] in `bin/node/testing/src/keyring.rs`!
///
/// [`sign`]: <../../testing/src/keyring.rs.html>
pub type SignedExtra = (
    frame_system::CheckNonZeroSender<Runtime>,
    frame_system::CheckSpecVersion<Runtime>,
    frame_system::CheckTxVersion<Runtime>,
    frame_system::CheckGenesis<Runtime>,
    frame_system::CheckEra<Runtime>,
    frame_system::CheckNonce<Runtime>,
    frame_system::CheckWeight<Runtime>,
    pallet_asset_tx_payment::ChargeAssetTxPayment<Runtime>,
);

/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
    fp_self_contained::UncheckedExtrinsic<Address, RuntimeCall, Signature, SignedExtra>;
/// The payload being signed in transactions.
pub type SignedPayload = generic::SignedPayload<RuntimeCall, SignedExtra>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic =
    fp_self_contained::CheckedExtrinsic<AccountId, RuntimeCall, SignedExtra, H160>;
/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
    Runtime,
    Block,
    frame_system::ChainContext<Runtime>,
    Runtime,
    AllPalletsWithSystem,
    Migrations,
>;

// All migrations executed on runtime upgrade as a nested tuple of types implementing
// `OnRuntimeUpgrade`.
type Migrations = ();

#[cfg(feature = "runtime-benchmarks")]
#[macro_use]
extern crate frame_benchmarking;

#[cfg(feature = "runtime-benchmarks")]
mod benches {
    frame_benchmarking::define_benchmarks!(
        [frame_benchmarking, BaselineBench::<Runtime>]
        [pallet_assets, Assets]
        [pallet_babe, Babe]
        [pallet_bags_list, VoterList]
        [pallet_balances, Balances]
        [pallet_bounties, Bounties]
        [pallet_child_bounties, ChildBounties]
        [pallet_collective, Council]
        [pallet_democracy, Democracy]
        [pallet_election_provider_multi_phase, ElectionProviderMultiPhase]
        [pallet_election_provider_support_benchmarking, EPSBench::<Runtime>]
        [pallet_elections_phragmen, Elections]
        [pallet_grandpa, Grandpa]
        [pallet_identity, Identity]
        [pallet_im_online, ImOnline]
        [pallet_indices, Indices]
        [pallet_multisig, Multisig]
        //[pallet_nomination_pools, NominationPoolsBench::<Runtime>]
        //[pallet_offences, OffencesBench::<Runtime>]
        [pallet_preimage, Preimage]
        [pallet_proxy, Proxy]
        [pallet_recovery, Recovery]
        [pallet_scheduler, Scheduler]
        //[pallet_session, SessionBench::<Runtime>]
        [pallet_staking, Staking]
        [frame_system, SystemBench::<Runtime>]
        [pallet_timestamp, Timestamp]
        [pallet_tips, Tips]
        [pallet_treasury, Treasury]
        [pallet_nfts, Nfts]
        [pallet_utility, Utility]
    );
}

pub struct TransactionConverter;
impl fp_rpc::ConvertTransaction<UncheckedExtrinsic> for TransactionConverter {
    fn convert_transaction(&self, transaction: pallet_ethereum::Transaction) -> UncheckedExtrinsic {
        UncheckedExtrinsic::new_unsigned(
            pallet_ethereum::Call::<Runtime>::transact { transaction }.into(),
        )
    }
}
impl fp_rpc::ConvertTransaction<sp_runtime::OpaqueExtrinsic> for TransactionConverter {
    fn convert_transaction(
        &self,
        transaction: pallet_ethereum::Transaction,
    ) -> sp_runtime::OpaqueExtrinsic {
        let extrinsic = UncheckedExtrinsic::new_unsigned(
            pallet_ethereum::Call::<Runtime>::transact { transaction }.into(),
        );
        let encoded = extrinsic.encode();
        sp_runtime::OpaqueExtrinsic::decode(&mut &encoded[..])
            .expect("Encoded extrinsic is always valid")
    }
}

impl fp_self_contained::SelfContainedCall for RuntimeCall {
    type SignedInfo = H160;

    fn is_self_contained(&self) -> bool {
        match self {
            RuntimeCall::Ethereum(call) => call.is_self_contained(),
            _ => false,
        }
    }

    fn check_self_contained(&self) -> Option<Result<Self::SignedInfo, TransactionValidityError>> {
        match self {
            RuntimeCall::Ethereum(call) => call.check_self_contained(),
            _ => None,
        }
    }

    fn validate_self_contained(
        &self,
        signed_info: &Self::SignedInfo,
        dispatch_info: &DispatchInfoOf<RuntimeCall>,
        len: usize,
    ) -> Option<TransactionValidity> {
        match self {
            RuntimeCall::Ethereum(call) => {
                call.validate_self_contained(signed_info, dispatch_info, len)
            },
            _ => None,
        }
    }

    fn pre_dispatch_self_contained(
        &self,
        info: &Self::SignedInfo,
        dispatch_info: &DispatchInfoOf<RuntimeCall>,
        len: usize,
    ) -> Option<Result<(), TransactionValidityError>> {
        match self {
            RuntimeCall::Ethereum(call) => {
                call.pre_dispatch_self_contained(info, dispatch_info, len)
            },
            _ => None,
        }
    }

    fn apply_self_contained(
        self,
        info: Self::SignedInfo,
    ) -> Option<sp_runtime::DispatchResultWithInfo<PostDispatchInfoOf<Self>>> {
        match self {
            call @ RuntimeCall::Ethereum(pallet_ethereum::Call::transact { .. }) => {
                Some(call.dispatch(RuntimeOrigin::from(
                    pallet_ethereum::RawOrigin::EthereumTransaction(info),
                )))
            },
            _ => None,
        }
    }
}

impl_runtime_apis! {
    impl sp_api::Core<Block> for Runtime {
        fn version() -> RuntimeVersion {
            VERSION
        }

        fn execute_block(block: Block) {
            Executive::execute_block(block);
        }

        fn initialize_block(header: &<Block as BlockT>::Header) {
            Executive::initialize_block(header)
        }
    }

    impl sp_api::Metadata<Block> for Runtime {
        fn metadata() -> OpaqueMetadata {
            OpaqueMetadata::new(Runtime::metadata().into())
        }

        fn metadata_at_version(version: u32) -> Option<OpaqueMetadata> {
            Runtime::metadata_at_version(version)
        }

        fn metadata_versions() -> sp_std::vec::Vec<u32> {
            Runtime::metadata_versions()
        }
    }

    impl sp_block_builder::BlockBuilder<Block> for Runtime {
        fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
            Executive::apply_extrinsic(extrinsic)
        }

        fn finalize_block() -> <Block as BlockT>::Header {
            Executive::finalize_block()
        }

        fn inherent_extrinsics(data: InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
            data.create_extrinsics()
        }

        fn check_inherents(block: Block, data: InherentData) -> CheckInherentsResult {
            data.check_extrinsics(&block)
        }
    }

    impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
        fn validate_transaction(
            source: TransactionSource,
            tx: <Block as BlockT>::Extrinsic,
            block_hash: <Block as BlockT>::Hash,
        ) -> TransactionValidity {
            Executive::validate_transaction(source, tx, block_hash)
        }
    }

    impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
        fn offchain_worker(header: &<Block as BlockT>::Header) {
            Executive::offchain_worker(header)
        }
    }

    impl sp_consensus_grandpa::GrandpaApi<Block> for Runtime {
        fn grandpa_authorities() -> sp_consensus_grandpa::AuthorityList {
            Grandpa::grandpa_authorities()
        }

        fn current_set_id() -> sp_consensus_grandpa::SetId {
            Grandpa::current_set_id()
        }

        fn submit_report_equivocation_unsigned_extrinsic(
            equivocation_proof: sp_consensus_grandpa::EquivocationProof<
                <Block as BlockT>::Hash,
                NumberFor<Block>,
            >,
            key_owner_proof: sp_consensus_grandpa::OpaqueKeyOwnershipProof,
        ) -> Option<()> {
            let key_owner_proof = key_owner_proof.decode()?;

            Grandpa::submit_unsigned_equivocation_report(
                equivocation_proof,
                key_owner_proof,
            )
        }

        fn generate_key_ownership_proof(
            _set_id: sp_consensus_grandpa::SetId,
            authority_id: GrandpaId,
        ) -> Option<sp_consensus_grandpa::OpaqueKeyOwnershipProof> {
            Historical::prove((sp_consensus_grandpa::KEY_TYPE, authority_id))
                .map(|p| p.encode())
                .map(sp_consensus_grandpa::OpaqueKeyOwnershipProof::new)
        }
    }

    impl pallet_nomination_pools_runtime_api::NominationPoolsApi<Block, AccountId, Balance> for Runtime {
        fn pending_rewards(who: AccountId) -> Balance {
            NominationPools::api_pending_rewards(who).unwrap_or_default()
        }

        fn points_to_balance(pool_id: pallet_nomination_pools::PoolId, points: Balance) -> Balance {
            NominationPools::api_points_to_balance(pool_id, points)
        }

        fn balance_to_points(pool_id: pallet_nomination_pools::PoolId, new_funds: Balance) -> Balance {
            NominationPools::api_balance_to_points(pool_id, new_funds)
        }
    }

    impl pallet_staking_runtime_api::StakingApi<Block, Balance> for Runtime {
        fn nominations_quota(balance: Balance) -> u32 {
            Staking::api_nominations_quota(balance)
        }
    }

    impl sp_consensus_babe::BabeApi<Block> for Runtime {
        fn configuration() -> sp_consensus_babe::BabeConfiguration {
            let epoch_config = Babe::epoch_config().unwrap_or(BABE_GENESIS_EPOCH_CONFIG);
            sp_consensus_babe::BabeConfiguration {
                slot_duration: Babe::slot_duration(),
                epoch_length: EpochDuration::get(),
                c: epoch_config.c,
                authorities: Babe::authorities().to_vec(),
                randomness: Babe::randomness(),
                allowed_slots: epoch_config.allowed_slots,
            }
        }

        fn current_epoch_start() -> sp_consensus_babe::Slot {
            Babe::current_epoch_start()
        }

        fn current_epoch() -> sp_consensus_babe::Epoch {
            Babe::current_epoch()
        }

        fn next_epoch() -> sp_consensus_babe::Epoch {
            Babe::next_epoch()
        }

        fn generate_key_ownership_proof(
            _slot: sp_consensus_babe::Slot,
            authority_id: sp_consensus_babe::AuthorityId,
        ) -> Option<sp_consensus_babe::OpaqueKeyOwnershipProof> {
            Historical::prove((sp_consensus_babe::KEY_TYPE, authority_id))
                .map(|p| p.encode())
                .map(sp_consensus_babe::OpaqueKeyOwnershipProof::new)
        }

        fn submit_report_equivocation_unsigned_extrinsic(
            equivocation_proof: sp_consensus_babe::EquivocationProof<<Block as BlockT>::Header>,
            key_owner_proof: sp_consensus_babe::OpaqueKeyOwnershipProof,
        ) -> Option<()> {
            let key_owner_proof = key_owner_proof.decode()?;

            Babe::submit_unsigned_equivocation_report(
                equivocation_proof,
                key_owner_proof,
            )
        }
    }

    impl sp_authority_discovery::AuthorityDiscoveryApi<Block> for Runtime {
        fn authorities() -> Vec<AuthorityDiscoveryId> {
            AuthorityDiscovery::authorities()
        }
    }

    impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index> for Runtime {
        fn account_nonce(account: AccountId) -> Index {
            System::account_nonce(account)
        }
    }

    impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<
        Block,
        Balance,
    > for Runtime {
        fn query_info(uxt: <Block as BlockT>::Extrinsic, len: u32) -> RuntimeDispatchInfo<Balance> {
            TransactionPayment::query_info(uxt, len)
        }
        fn query_fee_details(uxt: <Block as BlockT>::Extrinsic, len: u32) -> FeeDetails<Balance> {
            TransactionPayment::query_fee_details(uxt, len)
        }
        fn query_weight_to_fee(weight: Weight) -> Balance {
            TransactionPayment::weight_to_fee(weight)
        }
        fn query_length_to_fee(length: u32) -> Balance {
            TransactionPayment::length_to_fee(length)
        }
    }

    impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentCallApi<Block, Balance, RuntimeCall>
        for Runtime
    {
        fn query_call_info(call: RuntimeCall, len: u32) -> RuntimeDispatchInfo<Balance> {
            TransactionPayment::query_call_info(call, len)
        }
        fn query_call_fee_details(call: RuntimeCall, len: u32) -> FeeDetails<Balance> {
            TransactionPayment::query_call_fee_details(call, len)
        }
        fn query_weight_to_fee(weight: Weight) -> Balance {
            TransactionPayment::weight_to_fee(weight)
        }
        fn query_length_to_fee(length: u32) -> Balance {
            TransactionPayment::length_to_fee(length)
        }
    }

    impl sp_session::SessionKeys<Block> for Runtime {
        fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
            opaque::SessionKeys::generate(seed)
        }

        fn decode_session_keys(
            encoded: Vec<u8>,
        ) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
            opaque::SessionKeys::decode_into_raw_public_keys(&encoded)
        }
    }

    impl dbc_primitives_rpc_debug::DebugRuntimeApi<Block> for Runtime {
        fn trace_transaction(
            _extrinsics: Vec<<Block as BlockT>::Extrinsic>,
            _traced_transaction: &EthereumTransaction,
        ) -> Result<
            (),
            sp_runtime::DispatchError,
        > {
            #[cfg(feature = "evm-tracing")]
            {
                use dbc_evm_tracer::tracer::EvmTracer;
                // Apply the a subset of extrinsics: all the substrate-specific or ethereum
                // transactions that preceded the requested transaction.
                for ext in _extrinsics.into_iter() {
                    let _ = match &ext.0.function {
                        RuntimeCall::Ethereum(transact { transaction }) => {
                            if transaction == _traced_transaction {
                                EvmTracer::new().trace(|| Executive::apply_extrinsic(ext));
                                return Ok(());
                            } else {
                                Executive::apply_extrinsic(ext)
                            }
                        }
                        _ => {
                            Executive::apply_extrinsic(ext)
                        }
                    };
                }
                Err(sp_runtime::DispatchError::Other(
                    "Failed to find Ethereum transaction among the extrinsics.",
                ))
            }
            #[cfg(not(feature = "evm-tracing"))]
            Err(sp_runtime::DispatchError::Other(
                "Missing `evm-tracing` compile time feature flag.",
            ))
        }
        fn trace_block(
            _extrinsics: Vec<<Block as BlockT>::Extrinsic>,
            _known_transactions: Vec<H256>,
        ) -> Result<
            (),
            sp_runtime::DispatchError,
        > {
            #[cfg(feature = "evm-tracing")]
            {
                use dbc_evm_tracer::tracer::EvmTracer;
                let mut config = <Runtime as pallet_evm::Config>::config().clone();
                config.estimate = true;
                // Apply all extrinsics. Ethereum extrinsics are traced.
                for ext in _extrinsics.into_iter() {
                    match &ext.0.function {
                        RuntimeCall::Ethereum(transact { transaction }) => {
                            if _known_transactions.contains(&transaction.hash()) {
                                // Each known extrinsic is a new call stack.
                                EvmTracer::emit_new();
                                EvmTracer::new().trace(|| Executive::apply_extrinsic(ext));
                            } else {
                                let _ = Executive::apply_extrinsic(ext);
                            }
                        }
                        _ => {
                            let _ = Executive::apply_extrinsic(ext);
                        }
                    };
                }

                Ok(())
            }
            #[cfg(not(feature = "evm-tracing"))]
            Err(sp_runtime::DispatchError::Other(
                "Missing `evm-tracing` compile time feature flag.",
            ))
        }
    }
    impl dbc_primitives_rpc_txpool::TxPoolRuntimeApi<Block> for Runtime {
        fn extrinsic_filter(
            xts_ready: Vec<<Block as BlockT>::Extrinsic>,
            xts_future: Vec<<Block as BlockT>::Extrinsic>,
        ) -> TxPoolResponse {
            TxPoolResponse {
                ready: xts_ready
                .into_iter()
                .filter_map(|xt| match xt.0.function {
                    RuntimeCall::Ethereum(transact { transaction }) => Some(transaction),
                    _ => None,
                })
                .collect(),
                future: xts_future
                .into_iter()
                .filter_map(|xt| match xt.0.function {
                    RuntimeCall::Ethereum(transact { transaction }) => Some(transaction),
                    _ => None,
                })
                .collect(),
            }
        }
    }

    impl fp_rpc::EthereumRuntimeRPCApi<Block> for Runtime {
        fn chain_id() -> u64 {
            <Runtime as pallet_evm::Config>::ChainId::get()
        }

        fn account_basic(address: H160) -> EVMAccount {
            let (account, _) = EVM::account_basic(&address);
            account
        }

        fn gas_price() -> U256 {
            let (gas_price, _) = <Runtime as pallet_evm::Config>::FeeCalculator::min_gas_price();
            gas_price
        }

        fn account_code_at(address: H160) -> Vec<u8> {
            pallet_evm::AccountCodes::<Runtime>::get(address)
        }

        fn author() -> H160 {
            <pallet_evm::Pallet<Runtime>>::find_author()
        }

        fn storage_at(address: H160, index: U256) -> H256 {
            let mut tmp = [0u8; 32];
            index.to_big_endian(&mut tmp);
            pallet_evm::AccountStorages::<Runtime>::get(address, H256::from_slice(&tmp[..]))
        }

        fn call(
            from: H160,
            to: H160,
            data: Vec<u8>,
            value: U256,
            gas_limit: U256,
            max_fee_per_gas: Option<U256>,
            max_priority_fee_per_gas: Option<U256>,
            nonce: Option<U256>,
            estimate: bool,
            access_list: Option<Vec<(H160, Vec<H256>)>>,
        ) -> Result<pallet_evm::CallInfo, sp_runtime::DispatchError> {
            let config = if estimate {
                let mut config = <Runtime as pallet_evm::Config>::config().clone();
                config.estimate = true;
                Some(config)
            } else {
                None
            };
            let is_transactional = false;
            let validate = true;

            let mut estimated_transaction_len = data.len() +
                // to: 20
                // from: 20
                // value: 32
                // gas_limit: 32
                // nonce: 32
                // 1 byte transaction action variant
                // chain id 8 bytes
                // 65 bytes signature
                210;
            if max_fee_per_gas.is_some() {
                estimated_transaction_len += 32;
            }
            if max_priority_fee_per_gas.is_some() {
                estimated_transaction_len += 32;
            }
            if access_list.is_some() {
                estimated_transaction_len += access_list.encoded_size();
            }

            let gas_limit = gas_limit.min(u64::MAX.into()).low_u64();
            let without_base_extrinsic_weight = true;

            let (weight_limit, proof_size_base_cost) =
                match <Runtime as pallet_evm::Config>::GasWeightMapping::gas_to_weight(
                    gas_limit,
                    without_base_extrinsic_weight
                ) {
                    weight_limit if weight_limit.proof_size() > 0 => {
                        (Some(weight_limit), Some(estimated_transaction_len as u64))
                    }
                    _ => (None, None),
                };

            <Runtime as pallet_evm::Config>::Runner::call(
                from,
                to,
                data,
                value,
                gas_limit,
                max_fee_per_gas,
                max_priority_fee_per_gas,
                nonce,
                access_list.unwrap_or_default(),
                is_transactional,
                validate,
                weight_limit,
                proof_size_base_cost,
                config.as_ref().unwrap_or(<Runtime as pallet_evm::Config>::config()),
            ).map_err(|err| err.error.into())
        }

        fn create(
            from: H160,
            data: Vec<u8>,
            value: U256,
            gas_limit: U256,
            max_fee_per_gas: Option<U256>,
            max_priority_fee_per_gas: Option<U256>,
            nonce: Option<U256>,
            estimate: bool,
            access_list: Option<Vec<(H160, Vec<H256>)>>,
        ) -> Result<pallet_evm::CreateInfo, sp_runtime::DispatchError> {
            let config = if estimate {
                let mut config = <Runtime as pallet_evm::Config>::config().clone();
                config.estimate = true;
                Some(config)
            } else {
                None
            };
            let is_transactional = false;
            let validate = true;

            let mut estimated_transaction_len = data.len() +
                // to: 20
                // from: 20
                // value: 32
                // gas_limit: 32
                // nonce: 32
                // 1 byte transaction action variant
                // chain id 8 bytes
                // 65 bytes signature
                210;
            if max_fee_per_gas.is_some() {
                estimated_transaction_len += 32;
            }
            if max_priority_fee_per_gas.is_some() {
                estimated_transaction_len += 32;
            }
            if access_list.is_some() {
                estimated_transaction_len += access_list.encoded_size();
            }

            let gas_limit = if gas_limit > U256::from(u64::MAX) {
                u64::MAX
            } else {
                gas_limit.low_u64()
            };
            let without_base_extrinsic_weight = true;

            let (weight_limit, proof_size_base_cost) =
                match <Runtime as pallet_evm::Config>::GasWeightMapping::gas_to_weight(
                    gas_limit,
                    without_base_extrinsic_weight
                ) {
                    weight_limit if weight_limit.proof_size() > 0 => {
                        (Some(weight_limit), Some(estimated_transaction_len as u64))
                    }
                    _ => (None, None),
                };

            #[allow(clippy::or_fun_call)] // suggestion not helpful here
            <Runtime as pallet_evm::Config>::Runner::create(
                from,
                data,
                value,
                gas_limit,
                max_fee_per_gas,
                max_priority_fee_per_gas,
                nonce,
                access_list.unwrap_or_default(),
                is_transactional,
                validate,
                weight_limit,
                proof_size_base_cost,
                config.as_ref().unwrap_or(<Runtime as pallet_evm::Config>::config()),
            ).map_err(|err| err.error.into())
        }

        fn current_transaction_statuses() -> Option<Vec<TransactionStatus>> {
            pallet_ethereum::CurrentTransactionStatuses::<Runtime>::get()
        }

        fn current_block() -> Option<pallet_ethereum::Block> {
            pallet_ethereum::CurrentBlock::<Runtime>::get()
        }

        fn current_receipts() -> Option<Vec<pallet_ethereum::Receipt>> {
            pallet_ethereum::CurrentReceipts::<Runtime>::get()
        }

        fn current_all() -> (
            Option<pallet_ethereum::Block>,
            Option<Vec<pallet_ethereum::Receipt>>,
            Option<Vec<TransactionStatus>>,
        ) {
            (
                pallet_ethereum::CurrentBlock::<Runtime>::get(),
                pallet_ethereum::CurrentReceipts::<Runtime>::get(),
                pallet_ethereum::CurrentTransactionStatuses::<Runtime>::get(),
            )
        }

        fn extrinsic_filter(
            xts: Vec<<Block as BlockT>::Extrinsic>,
        ) -> Vec<EthereumTransaction> {
            xts.into_iter().filter_map(|xt| match xt.0.function {
                RuntimeCall::Ethereum(transact { transaction }) => Some(transaction),
                _ => None
            }).collect::<Vec<EthereumTransaction>>()
        }

        fn elasticity() -> Option<Permill> {
            None
        }

        fn gas_limit_multiplier_support() {}

        fn pending_block(
            xts: Vec<<Block as BlockT>::Extrinsic>,
        ) -> (Option<pallet_ethereum::Block>, Option<Vec<TransactionStatus>>) {
            for ext in xts.into_iter() {
                let _ = Executive::apply_extrinsic(ext);
            }

            Ethereum::on_finalize(System::block_number() + 1);

            (
                pallet_ethereum::CurrentBlock::<Runtime>::get(),
                pallet_ethereum::CurrentTransactionStatuses::<Runtime>::get()
            )
        }
    }

    impl fp_rpc::ConvertTransactionRuntimeApi<Block> for Runtime {
        fn convert_transaction(
            transaction: pallet_ethereum::Transaction
        ) -> <Block as BlockT>::Extrinsic {
            UncheckedExtrinsic::new_unsigned(
                pallet_ethereum::Call::<Runtime>::transact { transaction }.into(),
            )
        }
    }

    #[cfg(feature = "try-runtime")]
    impl frame_try_runtime::TryRuntime<Block> for Runtime {
        fn on_runtime_upgrade(checks: frame_try_runtime::UpgradeCheckSelect) -> (Weight, Weight) {
            // NOTE: intentional unwrap: we don't want to propagate the error backwards, and want to
            // have a backtrace here. If any of the pre/post migration checks fail, we shall stop
            // right here and right now.
            let weight = Executive::try_runtime_upgrade(checks).unwrap();
            (weight, RuntimeBlockWeights::get().max_block)
        }

        fn execute_block(
            block: Block,
            state_root_check: bool,
            signature_check: bool,
            select: frame_try_runtime::TryStateSelect
        ) -> Weight {
            // NOTE: intentional unwrap: we don't want to propagate the error backwards, and want to
            // have a backtrace here.
            Executive::try_execute_block(block, state_root_check, signature_check, select).unwrap()
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    impl frame_benchmarking::Benchmark<Block> for Runtime {
        fn benchmark_metadata(extra: bool) -> (
            Vec<frame_benchmarking::BenchmarkList>,
            Vec<frame_support::traits::StorageInfo>,
        ) {
            use frame_benchmarking::{baseline, Benchmarking, BenchmarkList};
            use frame_support::traits::StorageInfoTrait;

            // Trying to add benchmarks directly to the Session Pallet caused cyclic dependency
            // issues. To get around that, we separated the Session benchmarks into its own crate,
            // which is why we need these two lines below.
            // use pallet_session_benchmarking::Pallet as SessionBench;
            // use pallet_offences_benchmarking::Pallet as OffencesBench;
            use pallet_election_provider_support_benchmarking::Pallet as EPSBench;
            use frame_system_benchmarking::Pallet as SystemBench;
            use baseline::Pallet as BaselineBench;
            // use pallet_nomination_pools_benchmarking::Pallet as NominationPoolsBench;

            let mut list = Vec::<BenchmarkList>::new();
            list_benchmarks!(list, extra);

            let storage_info = AllPalletsWithSystem::storage_info();

            (list, storage_info)
        }

        fn dispatch_benchmark(
            config: frame_benchmarking::BenchmarkConfig
        ) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
            use frame_benchmarking::{baseline, Benchmarking, BenchmarkBatch,  TrackedStorageKey};

            // Trying to add benchmarks directly to the Session Pallet caused cyclic dependency
            // issues. To get around that, we separated the Session benchmarks into its own crate,
            // which is why we need these two lines below.
            // use pallet_session_benchmarking::Pallet as SessionBench;
            // use pallet_offences_benchmarking::Pallet as OffencesBench;
            use pallet_election_provider_support_benchmarking::Pallet as EPSBench;
            use frame_system_benchmarking::Pallet as SystemBench;
            use baseline::Pallet as BaselineBench;
            // use pallet_nomination_pools_benchmarking::Pallet as NominationPoolsBench;

            // impl pallet_session_benchmarking::Config for Runtime {}
            // impl pallet_offences_benchmarking::Config for Runtime {}
            impl pallet_election_provider_support_benchmarking::Config for Runtime {}
            impl frame_system_benchmarking::Config for Runtime {}
            impl baseline::Config for Runtime {}
            // impl pallet_nomination_pools_benchmarking::Config for Runtime {}

            use frame_support::traits::WhitelistedStorageKeys;
            let mut whitelist: Vec<TrackedStorageKey> = AllPalletsWithSystem::whitelisted_storage_keys();

            // Treasury Account
            // TODO: this is manual for now, someday we might be able to use a
            // macro for this particular key
            let treasury_key = frame_system::Account::<Runtime>::hashed_key_for(Treasury::account_id());
            whitelist.push(treasury_key.to_vec().into());

            let mut batches = Vec::<BenchmarkBatch>::new();
            let params = (&config, &whitelist);
            add_benchmarks!(params, batches);
            Ok(batches)
        }
    }

    impl simple_rpc_runtime_api::SimpleRpcApi<Block, AccountId, Balance> for Runtime {
        fn get_staker_identity(who: AccountId) -> Vec<u8> {
            SimpleRpc::get_staker_identity(who)
        }

        fn get_staker_list_info(cur_page: u64, per_page: u64) -> Vec<simple_rpc::StakerListInfo<Balance, AccountId>> {
            SimpleRpc::get_staker_list_info(cur_page, per_page)
        }
    }

    // Here implement custom runtime API.
    impl online_profile_runtime_api::OpRpcApi<Block, AccountId, Balance, BlockNumber> for Runtime {
        fn get_total_staker_num() -> u64 {
            OnlineProfile::get_total_staker_num()
        }

        fn get_op_info() -> online_profile::SysInfoDetail<Balance> {
            OnlineProfile::get_op_info()
        }

        fn get_staker_info(who: AccountId) -> online_profile::rpc_types::StakerInfo<Balance, BlockNumber, AccountId> {
            OnlineProfile::get_staker_info(who)
        }

        fn get_machine_list() -> dbc_support::live_machine::LiveMachine {
            OnlineProfile::get_machine_list()
        }

        fn get_machine_info(machine_id: MachineId) -> Option<dbc_support::machine_info::MachineInfo<AccountId, BlockNumber, Balance>> {
            OnlineProfile::get_machine_info(machine_id)
        }

        fn get_pos_gpu_info() -> Vec<(dbc_support::machine_type::Longitude, dbc_support::machine_type::Latitude, online_profile::PosInfo)> {
            OnlineProfile::get_pos_gpu_info()
        }

        fn get_machine_era_reward(machine_id: MachineId, era_index: EraIndex) -> Balance {
            OnlineProfile::get_machine_era_reward(machine_id, era_index)
        }

        fn get_machine_era_released_reward(machine_id: MachineId, era_index: EraIndex) -> Balance {
            OnlineProfile::get_machine_era_released_reward(machine_id, era_index)
        }

        fn get_stash_era_reward(stash: AccountId, era_index: EraIndex) -> Balance {
            OnlineProfile::get_stash_era_reward(stash, era_index)
        }

        fn get_stash_era_released_reward(stash: AccountId, era_index: EraIndex) -> Balance {
            OnlineProfile::get_stash_era_released_reward(stash, era_index)
        }

    }

    impl online_committee_runtime_api::OcRpcApi<Block, AccountId, BlockNumber, Balance> for Runtime {
        fn get_committee_machine_list(committee: AccountId) -> dbc_support::verify_online::OCCommitteeMachineList {
            OnlineCommittee::get_committee_machine_list(committee)
        }

        fn get_committee_ops(committee: AccountId, machine_id: MachineId) -> Option<online_committee::rpc::RpcOCCommitteeOps<BlockNumber, Balance>> {
            OnlineCommittee::get_committee_ops(committee, machine_id)
        }

        fn get_machine_committee_list(machine_id: MachineId) -> dbc_support::verify_online::OCMachineCommitteeList<AccountId, BlockNumber> {
            OnlineCommittee::get_machine_committee_list(machine_id)
        }
    }

    impl rent_machine_runtime_api::RmRpcApi<Block, AccountId, BlockNumber, Balance> for Runtime {
        fn get_rent_order(rent_id: RentOrderId) -> Option<dbc_support::rental_type::RentOrderDetail<AccountId, BlockNumber, Balance>> {
            RentMachine::get_rent_order(rent_id)
        }

        fn get_rent_list(renter: AccountId) -> Vec<RentOrderId> {
            RentMachine::get_rent_list(renter)
        }

        fn is_machine_renter(machine_id: MachineId, renter: AccountId) -> bool {
            RentMachine::is_machine_renter(machine_id, renter)
        }

        fn get_machine_rent_id(machine_id: MachineId) -> MachineGPUOrder {
            RentMachine::get_machine_rent_id(machine_id)
        }
    }

    impl committee_runtime_api::CmRpcApi<Block, AccountId> for Runtime {
        fn get_committee_list() -> committee::CommitteeList<AccountId> {
            Committee::get_committee_list()
        }
    }

    impl terminating_rental_runtime_api::IrRpcApi<Block, AccountId, Balance, BlockNumber> for Runtime {
        fn get_total_staker_num() -> u64 {
            TerminatingRental::get_total_staker_num()
        }

        fn get_staker_info(who: AccountId) -> terminating_rental::rpc_types::StakerInfo<Balance, BlockNumber, AccountId> {
            TerminatingRental::get_staker_info(who)
        }

        fn get_machine_list() -> dbc_support::live_machine::LiveMachine {
            TerminatingRental::get_machine_list()
        }

        fn get_machine_info(machine_id: MachineId) -> Option<dbc_support::machine_info::MachineInfo<AccountId, BlockNumber, Balance>> {
            TerminatingRental::get_machine_info(machine_id)
        }

        fn get_committee_machine_list(committee: AccountId) -> dbc_support::verify_online::OCCommitteeMachineList {
            TerminatingRental::get_committee_machine_list(committee)
        }

        fn get_committee_ops(committee: AccountId, machine_id: MachineId) -> Option<terminating_rental::rpc_types::RpcIRCommitteeOps<BlockNumber, Balance>> {
            TerminatingRental::get_committee_ops(committee, machine_id)
        }

        fn get_machine_committee_list(machine_id: MachineId) -> dbc_support::verify_online::OCMachineCommitteeList<AccountId, BlockNumber> {
            TerminatingRental::get_machine_committee_list(machine_id)
        }

        fn get_rent_order(rent_id: RentOrderId) -> Option<dbc_support::rental_type::RentOrderDetail<AccountId, BlockNumber, Balance>> {
            TerminatingRental::get_rent_order(rent_id)
        }

        fn get_rent_list(renter: AccountId) -> Vec<RentOrderId> {
            TerminatingRental::get_rent_list(renter)
        }

        fn is_machine_renter(machine_id: MachineId, renter: AccountId) -> bool {
            TerminatingRental::is_machine_renter(machine_id, renter)
        }

        fn get_machine_rent_id(machine_id: MachineId) -> MachineGPUOrder {
            TerminatingRental::get_machine_rent_id(machine_id)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frame_election_provider_support::NposSolution;
    use frame_support::traits::WhitelistedStorageKeys;
    use frame_system::offchain::CreateSignedTransaction;
    use sp_core::hexdisplay::HexDisplay;
    use sp_runtime::UpperOf;
    use std::collections::HashSet;

    #[test]
    fn check_whitelist() {
        let whitelist: HashSet<String> = AllPalletsWithSystem::whitelisted_storage_keys()
            .iter()
            .map(|e| HexDisplay::from(&e.key).to_string())
            .collect();

        // Block Number
        assert!(
            whitelist.contains("26aa394eea5630e07c48ae0c9558cef702a5c1b19ab7a04f536c519aca4983ac")
        );
        // Total Issuance
        assert!(
            whitelist.contains("c2261276cc9d1f8598ea4b6a74b15c2f57c875e4cff74148e4628f264b974c80")
        );
        // Execution Phase
        assert!(
            whitelist.contains("26aa394eea5630e07c48ae0c9558cef7ff553b5a9862a516939d82b3d3d8661a")
        );
        // Event Count
        assert!(
            whitelist.contains("26aa394eea5630e07c48ae0c9558cef70a98fdbe9ce6c55837576c60c7af3850")
        );
        // System Events
        assert!(
            whitelist.contains("26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7")
        );
        // System BlockWeight
        assert!(
            whitelist.contains("26aa394eea5630e07c48ae0c9558cef734abf5cb34d6244378cddbf18e849d96")
        );
    }

    #[test]
    fn validate_transaction_submitter_bounds() {
        fn is_submit_signed_transaction<T>()
        where
            T: CreateSignedTransaction<RuntimeCall>,
        {
        }

        is_submit_signed_transaction::<Runtime>();
    }

    #[test]
    fn perbill_as_onchain_accuracy() {
        type OnChainAccuracy =
			<<Runtime as pallet_election_provider_multi_phase::MinerConfig>::Solution as NposSolution>::Accuracy;
        let maximum_chain_accuracy: Vec<UpperOf<OnChainAccuracy>> = (0..MaxNominations::get())
            .map(|_| <UpperOf<OnChainAccuracy>>::from(OnChainAccuracy::one().deconstruct()))
            .collect();
        let _: UpperOf<OnChainAccuracy> =
            maximum_chain_accuracy.iter().fold(0, |acc, x| acc.checked_add(*x).unwrap());
    }

    #[test]
    fn call_size() {
        let size = core::mem::size_of::<RuntimeCall>();
        assert!(
			size <= 310,
			"size of RuntimeCall {} is more than 310 bytes: some calls have too big arguments, use Box to reduce the
			size of RuntimeCall.
			If the limit is too strong, maybe consider increase the limit.",
			size,
		);
    }

    #[test]
    fn check_block_time() {
        assert_eq!(DAYS, dbc_support::ONE_DAY, "make sure DAYS is equal to ONE_DAY");
    }
}
