// 1万卡一下 质押 = min(100000 DBC, 5w RMB 等值DBC)
// 1万卡以上，质押 = min(100000 * (10000/卡数), 5w RMB 等值DBC)
// TODO: 如果验证结果发现，绑定者与机器钱包地址不一致，则进行惩罚
// TODO: era结束时重新计算得分, 如果有会影响得分的改变，放到列表中，等era结束进行计算

#![cfg_attr(not(feature = "std"), no_std)]

use codec::EncodeLike;
use frame_support::{
    dispatch::DispatchResultWithPostInfo,
    pallet_prelude::*,
    traits::{Currency, Get, LockIdentifier, LockableCurrency, WithdrawReasons},
};
use frame_system::pallet_prelude::*;
use online_profile_machine::{LCOps, OCWOps};
use pallet_identity::Data;
use sp_runtime::{
    traits::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, Zero},
    Perbill, SaturatedConversion,
};
use sp_std::{collections::btree_map::BTreeMap, collections::vec_deque::VecDeque, prelude::*, str};

pub mod grade_inflation;
pub mod op_types;
pub mod rpc_types;

pub use op_types::*;
pub use rpc_types::*;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub const PALLET_LOCK_ID: LockIdentifier = *b"oprofile";
pub const REPORTER_LOCK_ID: LockIdentifier = *b"reporter";
pub const MAX_UNLOCKING_CHUNKS: usize = 32;

// 惩罚发生后，有48小时的时间提交议案取消惩罚
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MachinesSlash<AccountId, BlockNumber, Balance> {
    pub reporter: AccountId,
    pub reporter_time: BlockNumber,
    pub slash_reason: SlashReason,
    pub slash_amount: Balance,
    pub unapplied_slash: u32, // 记录多少个阶段的奖励被扣除
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum SlashReason {
    MinuteOffline3, // 3min 掉线
    MinuteOffline7, // 7min 掉线
    DaysOffline2,   // 2days 掉线
    DaysOffline5,   // 5days 掉线
}

impl Default for SlashReason {
    fn default() -> Self {
        SlashReason::MinuteOffline3
    }
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct CommitteeMachine<Balance> {
    pub machine_id: Vec<MachineId>, //用户绑定的所有机器，不与机器状态有关
    pub total_calc_points: u64,
    pub total_gpu_num: u64,
    pub total_reward: Balance,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MachineInfo<AccountId: Ord, BlockNumber> {
    pub machine_owner: AccountId, // 允许用户绑定跟自己机器ID不一样的，TODO: 奖励发放给machine_owner
    pub bonding_height: BlockNumber, // 记录机器第一次绑定的时间, TODO: 改为current_slot
    pub machine_status: MachineStatus,
    pub ocw_machine_info: MachineInfoDetail,
    pub machine_grade: u64, // TODO: 添加machine_info时，加上machine_grade
    pub machine_price: u64, // TODO: 设置3080的分数对应的价格为1000元，其他机器的价格根据3080的进行计算
    pub committee_confirm: BTreeMap<AccountId, CommitteeConfirmation<AccountId, BlockNumber>>, //记录委员会提交的机器打分
    pub reward_committee: Vec<AccountId>, // 列表中的委员将分得用户奖励
    pub reward_deadline: BlockNumber, // 列表中委员分得奖励结束时间 , TODO: 绑定时间改为current_slot比较好
}

// 委员会提交的机器配置信息
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MachineInfoDetail {}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct CommitteeConfirmation<AccountId, BlockNumber> {
    pub committee: AccountId,
    pub confirm_time: BlockNumber,
    pub is_confirmed: bool,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum MachineStatus {
    Bonding,
    Booked,
    WaitingHash,
    Bonded,
    WaitingFulfill, // 等待补交罚款
}

impl Default for MachineStatus {
    fn default() -> Self {
        MachineStatus::Bonding
    }
}

// 只保存正常声明周期的Machine,删除掉的/绑定失败的不保存在该变量中
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct LiveMachine {
    pub bonding_machine: Vec<MachineId>, // 用户质押DBC并绑定机器，机器ID添加到本字段
    pub ocw_confirming_machine: Vec<MachineId>, // ocw把bonding_machine移动到这个列表，表示一个ocw正在处理的机器id，避免多个ocw同时处理一个机器Id
    pub ocw_confirmed_machine: Vec<MachineId>, // OCW从bonding_machine中读取机器ID，确认之后，添加到本字段。该状态可以由lc分配订单
    pub booked_machine: Vec<MachineId>, // 当机器已经全部分配了委员会，则变为该状态。若lc确认机器失败(认可=不认可)则返回上一状态，重新分派订单
    pub waiting_hash: Vec<MachineId>, // 当全部委员会添加了全部confirm hash之后，机器添加到waiting_hash，这时，用户可以添加confirm_raw
    pub bonded_machine: Vec<MachineId>, // 当全部委员会添加了confirm_raw之后，机器被成功绑定，变为bonded_machine状态
}

impl LiveMachine {
    // 检查machine_id是否存
    fn machine_id_exist(&self, machine_id: &MachineId) -> bool {
        if let Ok(_) = self.bonding_machine.binary_search(machine_id) {
            return true;
        }
        if let Ok(_) = self.ocw_confirmed_machine.binary_search(machine_id) {
            return true;
        }
        if let Ok(_) = self.booked_machine.binary_search(machine_id) {
            return true;
        }
        if let Ok(_) = self.waiting_hash.binary_search(machine_id) {
            return true;
        }
        if let Ok(_) = self.bonded_machine.binary_search(machine_id) {
            return true;
        }
        false
    }

    fn add_machine_id(a_field: &mut Vec<MachineId>, machine_id: MachineId) {
        if let Err(index) = a_field.binary_search(&machine_id) {
            a_field.insert(index, machine_id);
        }
    }

    fn rm_machine_id(a_field: &mut Vec<MachineId>, machine_id: &MachineId) {
        if let Ok(index) = a_field.binary_search(machine_id) {
            a_field.remove(index);
        }
    }
}

type BalanceOf<T> =
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[rustfmt::skip]
#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + dbc_price_ocw::Config + random_num::Config + pallet_identity::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Currency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;
        type BondingDuration: Get<EraIndex>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    // 存储用户机器在线收益
    #[pallet::type_value]
    pub fn HistoryDepthDefault<T: Config>() -> u32 {
        150
    }

    #[pallet::storage]
    #[pallet::getter(fn history_depth)]
    pub(super) type HistoryDepth<T: Config> =
        StorageValue<_, u32, ValueQuery, HistoryDepthDefault<T>>;

    // 用户线性释放的天数:
    // 25%收益当天释放；75%在150天线性释放
    #[pallet::type_value]
    pub(super) fn ProfitReleaseDurationDefault<T: Config>() -> u64 {
        150
    }

    // OCW获取机器信息时，超时次数
    #[pallet::storage]
    pub(super) type ProfitReleaseDuration<T: Config> = StorageValue<_, u64, ValueQuery, ProfitReleaseDurationDefault<T>>;

    // 存储机器的最小质押量，单位DBC, 默认为100000DBC
    #[pallet::storage]
    #[pallet::getter(fn stake_per_gpu)]
    pub(super) type StakePerGPU<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    // 存储每个机器质押的等值USD上限, 单位 1x10^6 USD
    #[pallet::storage]
    #[pallet::getter(fn stake_usd_limit)]
    pub(super) type StakeUSDLimit<T: Config> = StorageValue<_, u64, ValueQuery>;

    // 存储每个用户在该模块中的总质押量
    #[pallet::storage]
    #[pallet::getter(fn user_total_stake)]
    pub(super) type UserTotalStake<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

    // 存储当前每卡质押数量,
    #[pallet::storage]
    #[pallet::getter(fn cur_stake_per_gpu)]
    pub(super) type CurStakePerGPU<T> = StorageValue<_, BalanceOf<T>>;

    // 当前系统中工作的GPU数量
    #[pallet::storage]
    #[pallet::getter(fn total_gpu_num)]
    pub(super) type TotalGPUNum<T: Config> = StorageValue<_, u64, ValueQuery>;

    // 当前系统中矿工数
    #[pallet::storage]
    #[pallet::getter(fn total_staker)]
    pub(super) type TotalStaker<T: Config> = StorageValue<_, u64, ValueQuery>;

    // 当前系统中总算力点数
    #[pallet::storage]
    #[pallet::getter(fn total_calc_points)]
    pub(super) type TotalCalcPoints<T: Config> = StorageValue<_, u64, ValueQuery>;

    // 机器质押的DBC总数量
    #[pallet::storage]
    #[pallet::getter(fn total_stake)]
    pub(super) type TotalStake<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    // 机器的详细信息,只有当所有奖励领取完才能删除该变量?
    #[pallet::storage]
    #[pallet::getter(fn machines_info)]
    pub type MachinesInfo<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        MachineId,
        MachineInfo<T::AccountId, T::BlockNumber>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn user_machines)]
    pub(super) type UserMachines<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, CommitteeMachine<BalanceOf<T>>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn temp_account)]
    pub(super) type TempAccount<T: Config> = StorageValue<_, Vec<T::AccountId>, ValueQuery>;

    // 存储活跃的机器
    #[pallet::storage]
    #[pallet::getter(fn live_machines)]
    pub type LiveMachines<T: Config> = StorageValue<_, LiveMachine, ValueQuery>;

    /// Map from all (unlocked) "controller" accounts to the info regarding the staking.
    #[pallet::storage]
    #[pallet::getter(fn ledger)]
    pub type Ledger<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        MachineId,
        Option<StakingLedger<T::AccountId, BalanceOf<T>>>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn eras_reward_balance)]
    pub(super) type ErasRewardBalance<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        EraIndex,
        EraRewardBalance<T::AccountId, BalanceOf<T>>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn reward_start_height)]
    pub type RewardStartHeight<T: Config> = StorageValue<_, T::BlockNumber, ValueQuery>;

    // 奖励数量：第一个月为1亿，之后每个月为3300万
    // 2年10个月之后，奖励数量减半，之后再五年，奖励减半
    #[pallet::storage]
    #[pallet::getter(fn reward_per_year)]
    pub(super) type RewardPerYear<T> = StorageValue<_, BalanceOf<T>>;

    // 等于RewardPerYear * (era_duration / year_duration)
    #[pallet::storage]
    #[pallet::getter(fn eras_staker_reward)]
    pub(super) type ErasStakerReward<T> =
        StorageMap<_, Blake2_128Concat, EraIndex, Option<BalanceOf<T>>>;

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(_block_number: T::BlockNumber) {
            // FIXME
            // let a = Self::cur_stake_per_gpu();
            // let cur_stake_per_gpu = Self::calc_stake_amount();
            // if let Some(amount) = cur_stake_per_gpu {
            //     CurStakePerGPU::<T>::put(Some(amount));
            // } else {
            //     CurStakePerGPU::<T>::put(None);
            // }

            // CurStakePerGPU::<T>::put(cur_stake_per_gpu);
            // if (block_number.saturated_into::<u64>() + 1) / T::BlockPerEra::get() as u64 == 0 {
            //     Self::end_era()
            // }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // 实现当达到5000卡时，开启奖励
        #[pallet::weight(0)]
        pub fn set_reward_start_height(origin: OriginFor<T>, reward_start_height: T::BlockNumber) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            RewardStartHeight::<T>::put(reward_start_height);
            Ok(().into())
        }

        // 设置单卡质押数量
        #[pallet::weight(0)]
        pub fn set_gpu_stake(origin: OriginFor<T>, stake_per_gpu: BalanceOf<T>) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            StakePerGPU::<T>::put(stake_per_gpu);
            Ok(().into())
        }

        // 设置单GPU质押量换算成USD的上限
        #[pallet::weight(0)]
        pub fn set_stake_usd_limit(origin: OriginFor<T>, stake_usd_limit: u64) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            StakeUSDLimit::<T>::put(stake_usd_limit);
            Ok(().into())
        }

        // TODO: 删除该方法。该方法用来添加临时接口，以返回所有质押者的rpc接口
        #[pallet::weight(0)]
        pub fn add_temp_account(origin: OriginFor<T>, new_account: T::AccountId) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let mut accounts = Self::temp_account();
            accounts.push(new_account);
            TempAccount::<T>::put(accounts);
            Ok(().into())
        }

        // 将machine_id添加到绑定队列,之后ocw工作，验证机器ID与钱包地址是否一致
        // 绑定需要质押first_bond_stake数量的DBC
        #[pallet::weight(10000)]
        pub fn bond_machine(origin: OriginFor<T>, machine_owner: T::AccountId, machine_id: MachineId) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;

            // 用户第一次绑定机器需要质押的数量
            let first_bond_stake = Self::stake_per_gpu();

            // 资金检查,确保机器还没有被绑定过
            ensure!(<T as Config>::Currency::free_balance(&controller) > first_bond_stake, Error::<T>::BalanceNotEnough);
            let mut live_machines = Self::live_machines();
            ensure!(!live_machines.machine_id_exist(&machine_id), Error::<T>::MachineIdExist);

            // 添加到用户的机器列表
            let mut user_machines = Self::user_machines(&controller);
            if let Err(index) = user_machines.machine_id.binary_search(&machine_id) {
                user_machines.machine_id.insert(index, machine_id.clone());
                UserMachines::<T>::insert(&controller, user_machines);
            } else {
                return Err(Error::<T>::MachineInUserBonded.into());
            }

            // 添加到LiveMachine的bonding_machine字段
            LiveMachine::add_machine_id(&mut live_machines.bonding_machine, machine_id.clone());
            LiveMachines::<T>::put(live_machines);

            // 初始化MachineInfo, 并添加到MachinesInfo
            let machine_info = MachineInfo {
                machine_owner: machine_owner,
                bonding_height: <frame_system::Module<T>>::block_number(),
                ..Default::default()
            };
            MachinesInfo::<T>::insert(&machine_id, machine_info);

            // 直接初始化Ledger, 如果绑定失败，则调用unbond方法，进行自动解邦.
            // let current_era = <pallet_staking::Module<T>>::current_era().unwrap_or(0);
            let current_era: u32 = <random_num::Module<T>>::current_slot_height().saturated_into::<u32>();
            let history_depth = Self::history_depth();
            let last_reward_era = current_era.saturating_sub(history_depth);
            let item = StakingLedger {
                stash: controller.clone(),
                total: first_bond_stake,
                active: first_bond_stake,
                unlocking: vec![],
                claimed_rewards: (last_reward_era..current_era).collect(),
                released_rewards: 0u32.into(),
                upcoming_rewards: VecDeque::new(),
            };

            // 更新质押和Ledger
            Self::add_user_total_stake(&controller, first_bond_stake);
            Ledger::<T>::insert(controller.clone(), machine_id.clone(), Some(item));

            Self::deposit_event(Event::BondMachine(controller, machine_id, first_bond_stake));

            Ok(().into())
        }

        // 当用户被罚款后，需要补充质押金额
        #[pallet::weight(10000)]
        fn fulfill_bond(origin: OriginFor<T>, machine_id: MachineId) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;

            let mut ledger = Self::ledger(&controller, &machine_id).ok_or(Error::<T>::LedgerNotFound)?;
            let user_balance = <T as pallet::Config>::Currency::free_balance(&controller);

            let machine_stake_need = Self::calc_machine_stake_need(&machine_id);
            ensure!(machine_stake_need > user_balance, Error::<T>::InsufficientValue);

            if let Some(extra_stake) = machine_stake_need.checked_sub(&ledger.total) {
                ledger.total += extra_stake;
                ledger.active += extra_stake;

                Self::deposit_event(Event::AddBonded(
                    controller.clone(),
                    machine_id.clone(),
                    extra_stake,
                ));

                // 更新质押和Ledger
                Self::add_user_total_stake(&controller, extra_stake);
                Ledger::<T>::insert(controller, machine_id, Some(ledger));
            }

            Ok(().into())
        }

        // TODO: 确定退出时机: 当前无法确定机器的租用状态
        // 当用户想要机器从中退出时，可以调用unbond，来取出质押的金额
        // 当在线时长达到365天，并且10天没有人租用时，可以下线
        #[pallet::weight(10000)]
        fn unbond(origin: OriginFor<T>, machine_id: MachineId) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;

            // TODO: 增加这个功能
            let amount: BalanceOf<T> = 0u32.into();

            let mut ledger = Self::ledger(&controller, &machine_id).ok_or(Error::<T>::LedgerNotFound)?;
            ensure!(ledger.unlocking.len() < crate::MAX_UNLOCKING_CHUNKS, Error::<T>::NoMoreChunks);

            let mut value = amount.min(ledger.active);
            if !value.is_zero() {
                ledger.active -= value;

                if ledger.active < <T as Config>::Currency::minimum_balance() {
                    value += ledger.active;
                    ledger.active = Zero::zero();
                }

                // let era = <pallet_staking::Module<T>>::current_era().unwrap_or(0) + <T as pallet::Config>::BondingDuration::get();
                let era = <random_num::Module<T>>::current_era() + <T as pallet::Config>::BondingDuration::get();
                ledger.unlocking.push(UnlockChunk { value, era });

                // Self::update_ledger(&controller, &machine_id, &ledger);
                Self::deposit_event(Event::RemoveBonded(controller, machine_id, value));
            }

            Ok(().into())
        }

        // 当存在unbond金额时，到期后，用户可以取出该金额
        #[pallet::weight(10000)]
        fn withdraw_unbonded(origin: OriginFor<T>, machine_id: MachineId) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;

            let mut ledger = Self::ledger(&controller, &machine_id).ok_or(Error::<T>::LedgerNotFound)?;

            let old_total = ledger.total;
            let current_era = <random_num::Module<T>>::current_era();
            ledger = ledger.consolidate_unlock(current_era);

            if ledger.unlocking.is_empty() && ledger.active <= <T as pallet::Config>::Currency::minimum_balance() {
                // 清除ledger相关存储
                <T as pallet::Config>::Currency::remove_lock(crate::PALLET_LOCK_ID, &controller);
            } else {
                // Self::update_ledger(&controller, &machine_id, &ledger);
            }

            if ledger.total < old_total {
                let value = old_total - ledger.total;
                Self::deposit_event(Event::Withdrawn(controller, machine_id, value));
            }

            Ok(().into())
        }

        // // TODO: 重新实现这个函数
        // #[pallet::weight(10000)]
        // pub fn rm_bonded_machine(
        //     origin: OriginFor<T>,
        //     _machine_id: MachineId,
        // ) -> DispatchResultWithPostInfo {
        //     let _user = ensure_signed(origin)?;
        //     Ok(().into())
        // }

        // 允许其他用户给别人的机器领取奖励
        #[pallet::weight(10000)]
        pub fn payout_all_rewards(origin: OriginFor<T>, controller: T::AccountId) -> DispatchResultWithPostInfo {
            ensure_signed(origin)?;
            ensure!(UserMachines::<T>::contains_key(&controller), Error::<T>::NotMachineController);

            let user_machines = Self::user_machines(&controller);
            for machine_id in user_machines.machine_id.iter() {
                return Self::do_payout(controller.clone(), machine_id);
            }

            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn payout_rewards(origin: OriginFor<T>, controller: T::AccountId, machine_id: MachineId) -> DispatchResultWithPostInfo {
            ensure_signed(origin)?;
            ensure!(UserMachines::<T>::contains_key(&controller), Error::<T>::NotMachineController);

            Self::do_payout(controller, &machine_id)
        }

        #[pallet::weight(0)]
        pub fn cancle_slash(origin: OriginFor<T>, _machine_id: MachineId) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        BondMachine(T::AccountId, MachineId, BalanceOf<T>),
        AddBonded(T::AccountId, MachineId, BalanceOf<T>),
        RemoveBonded(T::AccountId, MachineId, BalanceOf<T>),
        DonationReceived(T::AccountId, BalanceOf<T>, BalanceOf<T>),
        FundsAllocated(T::AccountId, BalanceOf<T>, BalanceOf<T>),
        Withdrawn(T::AccountId, MachineId, BalanceOf<T>),
        ClaimRewards(T::AccountId, MachineId, BalanceOf<T>),
        ReporterStake(T::AccountId, MachineId, BalanceOf<T>),
    }

    #[pallet::error]
    pub enum Error<T> {
        MachineIdExist,
        MachineIdNotBonded,
        MachineInBonded,
        MachineInBonding,
        MachineInBooking,
        MachineInBooked,
        MachineInUserBonded,
        MachineStatusNotNormal,
        TokenNotBonded,
        BondedNotEnough,
        HttpDecodeError,
        BalanceNotEnough,
        NotMachineOwner,
        LedgerNotFound,
        NoMoreChunks,
        AlreadyAddedMachine,
        InsufficientValue,
        IndexOutOfRange,
        InvalidEraToReward,
        AccountNotSame,
        NotInBookingList,
        StakeNotEnough,
        NotMachineController,
        DBCPriceUnavailable,
    }
}

#[rustfmt::skip]
impl<T: Config> Pallet<T> {
    // 质押DBC机制：[0, 10000] GPU: 100000 DBC per GPU
    // (10000, +) -> min( 100000 * 10000 / (10000 + n), 5w RMB DBC )
    pub fn calc_stake_amount() -> Option<BalanceOf<T>> {
        let base_stake = Self::stake_per_gpu(); // 10_0000 DBC
        let one_dbc: BalanceOf<T> = 1000_000_000_000_000u64.saturated_into();

        // 计算5w RMB 等值DBC数量
        // dbc_amount = dbc_stake_usd_limit * 10^15 / dbc_price
        let dbc_price = <dbc_price_ocw::Module<T>>::avg_price();
        let stake_usd_limit: BalanceOf<T> = Self::stake_usd_limit().saturated_into();
        if let None = dbc_price {
            return None;
        }
        let dbc_price: BalanceOf<T>= dbc_price.unwrap().saturated_into();
        let dbc_amount = one_dbc.checked_mul(&stake_usd_limit);
        if let None = dbc_amount {
            return None
        }
        let dbc_amount = dbc_amount.unwrap();
        let dbc_amount = dbc_amount.checked_div(&dbc_price);
        if let None = dbc_amount {
            return None
        }
        let dbc_amount = dbc_amount.unwrap();

        // 当前成功加入系统的GPU数量
        let total_gpu_num = Self::total_gpu_num();

        if total_gpu_num <= 10_000 {
            // GPU数量小于10_000时，直接返回base_saturated_into() satura
            return Some(base_stake.min(dbc_amount));
        }

        // 当GPU数量大于10_000时
        // 100_000 * 10000 / gpu_num
        let dbc_amount2 = Perbill::from_rational_approximation(10_000u64, total_gpu_num) * base_stake;
        return Some(dbc_amount2.min(dbc_amount));
    }

    // TODO: 根据GPU数量修改需要的质押数量
    fn calc_machine_stake_need(_machine_id: &MachineId) -> BalanceOf<T> {
        0u32.into()
    }

    pub fn do_payout(controller: T::AccountId, machine_id: &MachineId) -> DispatchResultWithPostInfo {
        // 根据解锁数量打币给用户
        let mut ledger = Self::ledger(controller.clone(), machine_id).ok_or(Error::<T>::LedgerNotFound)?;
        let can_claim = ledger.released_rewards;

        // 检查机器是否处于正常状态
        let machine_info = Self::machines_info(machine_id.to_vec());
        if machine_info.machine_status != MachineStatus::Bonded {
            return Err(Error::<T>::NotMachineController.into());
        }

        // 计算给用户的部分和给委员会的部分
        // 99%奖金给机器控制者，1%奖励给用户

        // 判断委员会是否应该获得奖励
        let current_slot_height: u64 = <random_num::Module<T>>::current_slot_height().saturated_into::<u64>();
        if machine_info.reward_deadline.saturated_into::<u64>() >= current_slot_height {
            // 委员会也分得奖励
            let to_controller = Perbill::from_rational_approximation(99u64, 100) * can_claim;
            let to_committees= can_claim - to_controller;
            let to_one_committee = Perbill::from_rational_approximation(1u64, machine_info.reward_committee.len() as u64) * to_committees;

            for a_committee in machine_info.reward_committee.iter() {
                <T as Config>::Currency::deposit_into_existing(&a_committee, to_one_committee)?;
                Self::deposit_event(Event::ClaimRewards((*a_committee).clone(), machine_id.to_vec(), to_one_committee));
            }

            <T as Config>::Currency::deposit_into_existing(&controller, to_controller)?;
            Self::deposit_event(Event::ClaimRewards(controller.clone(), machine_id.to_vec(), to_controller));
        } else {
            // 奖励全分给控制者
            <T as Config>::Currency::deposit_into_existing(&controller, can_claim)?;
            Self::deposit_event(Event::ClaimRewards(controller.clone(), machine_id.to_vec(), can_claim));
        }

        // 更新已解压数量
        ledger.released_rewards = 0u32.into();
        Ledger::<T>::insert(controller, machine_id, Some(ledger));

        Ok(().into())
    }

    // 获取机器最近n天的奖励
    pub fn remaining_n_eras_reward(_machine_id: MachineId, _recent_eras: u32) -> BalanceOf<T> {
        return 0u32.into();
    }

    // // 被惩罚了应该是什么状态让用户无法处理其奖金，
    // pub fn slash_n_eras_reward(machine_id: MachineId, recent_eras: u32) -> BalanceOf<T> {
    //     return 0u32.into();
    // }

    // fn confirmed_committee(id: MachineId) -> BTreeSet<AccountId> {
    //     let machines_info = Self::machines_info(&id);

    // }

    // TODO: 在这四个函数中，更新machine的状态

    // 为机器增加奖励
    pub fn reward_by_ids(era_index: u32, validators_balance: impl IntoIterator<Item = (T::AccountId, BalanceOf<T>)>) {
        <ErasRewardBalance<T>>::mutate(era_index, |era_rewards| {
            for (validator, grades) in validators_balance.into_iter() {
                *era_rewards.individual.entry(validator).or_default() += grades;
                era_rewards.total += grades;
            }
        });
    }

    // 更新以下信息:
    // [机器质押信息] TODO: 质押一定数量之后再解邦
    // [机器质押代币总数量] (这就是为什么需要14天解绑，保证今天解绑不会影响今天总质押)
    // [机器总打分信息]
    // [机器分别的打分信息]

    // 可能在一个Era中再次更新的信息:
    // [机器打分信息]: 如果有减少，则立即更新，如果有增加，则下一个时间更新
    // [机器总打分信息]: 如果某一个机器打分减少，则总打分信息也会变化
    // TODO: 在start_era 的时候，更新打分信息,记录质押信息,可以加一个全局锁，将这段函数放在OCW中完成
    // TODO: 清理未收集的数据
    // TODO: 触发惩罚
    fn _start_era() {
        let _current_era = <random_num::Module<T>>::current_era();
        // let bonded_machine_id = Self::bonded_machine_id();

        // for a_machine_id in bonded_machine_id.iter() {
        //     let machine_grade = 1;
        //     // <ErasMachineGrades<T>>::insert(&current_era, "machine_id", machine_grade);
        // }

        // let a_machine_grade = 1;
    }

    fn _end_era() {
        // TODO: 参考staking模块的end_era
        // grade_inflation::compute_stake_grades(machine_price, staked_in, machine_grade)
    }

    // 计算每天的奖励，25%添加到用户的余额，75%在150天线性释放，TODO: 一部分释放给委员会
    fn _add_daily_reward(controller: T::AccountId, machine_id: MachineId, amount: BalanceOf<T>) {
        let ledger = Self::ledger(&controller, &machine_id);
        // ledger 在bond成功会初始化，若不存在，则直接返回
        if let None = ledger {
            return
        }
        let mut ledger = ledger.unwrap();

        // 将今天的奖励pop出，并增加到released_rewrads
        if let Some(today_released_reward) = ledger.upcoming_rewards.pop_front() {
            ledger.released_rewards += today_released_reward;
        }

        // 将amount 的25%直接添加到用户的资金
        let released_now = Perbill::from_rational_approximation(25u64, 100) * amount;
        let released_daily = Perbill::from_rational_approximation(5u64, 1000) * amount; // 接下来150天,每天释放奖励的千分之五

        ledger.released_rewards += released_now;

        // 75%添加到用户的剩下150天的余额中
        let unreleased_days = ledger.upcoming_rewards.len();
        if unreleased_days < 150 {
            let mut future_release = vec![released_daily; 150];
            for i in 0..unreleased_days {
                future_release[i] += ledger.upcoming_rewards[i];
            }
            ledger.upcoming_rewards = future_release.into_iter().collect();
        } else {
            for i in 0..150 {
                ledger.upcoming_rewards[i] += released_daily;
            }
        }


        Ledger::<T>::insert(controller, machine_id, Some(ledger));
        // Self::update_ledger(&controller, &machine_id, &ledger);
    }

    // 扣除n天剩余奖励
    fn _slash_nday_reward(_controller: T::AccountId, _machine_id: MachineId, _amount: BalanceOf<T>) {

    }

    fn _validator_slash(){}

    // TODO: update_ledger 改成add_stake, reduce_stake + update Ledger
    // // 更新用户的质押的ledger
    // fn update_ledger(controller: &T::AccountId, machine_id: &MachineId, ledger: &StakingLedger<T::AccountId, BalanceOf<T>>) {
    //     <T as pallet::Config>::Currency::set_lock(PALLET_LOCK_ID, &ledger.stash, ledger.total, WithdrawReasons::all());
    //     Ledger::<T>::insert(controller, machine_id, Some(ledger));
    // }

    fn add_user_total_stake(controller: &T::AccountId, amount: BalanceOf<T>) {
        let current_stake = Self::user_total_stake(controller);
        let next_stake = current_stake.checked_add(&amount);

        if let None = next_stake {
            return;
        }
        let next_stake = next_stake.unwrap();

        <T as pallet::Config>::Currency::set_lock(PALLET_LOCK_ID, controller, next_stake, WithdrawReasons::all());
    }

    fn _reduce_total_stake(controller: &T::AccountId, amount: BalanceOf<T>) {
        let current_stake = Self::user_total_stake(controller);
        let next_stake = current_stake.checked_sub(&amount);
        if let None = next_stake {
            return;
        }
        let next_stake = next_stake.unwrap();

        <T as pallet::Config>::Currency::set_lock(PALLET_LOCK_ID, controller, next_stake, WithdrawReasons::all());
    }
}

// TODO: 当request超过三次请求失败时，允许用户退回资金。
// 当地址不一致，则扣除用户资金
// online-profile-ocw可以执行的操作
impl<T: Config> OCWOps for Pallet<T> {
    type MachineId = MachineId;
    type AccountId = T::AccountId;

    // ocw启动时，将所有需要验证的机器放到验证区，防止其他worker重复验证
    fn ocw_booking_machine() -> Vec<MachineId> {
        let mut live_machines = Self::live_machines();

        let bonding_item = live_machines.bonding_machine.clone();

        for a_machine in bonding_item.iter() {
            LiveMachine::rm_machine_id(&mut live_machines.bonding_machine, a_machine);
            LiveMachine::add_machine_id(
                &mut live_machines.ocw_confirming_machine,
                a_machine.to_vec(),
            );
        }

        return bonding_item;
    }

    // 将machine_id从LiveMachines.bonding_machine中移除
    fn rm_booked_id(id: &MachineId) {
        let mut live_machines = Self::live_machines();
        LiveMachine::rm_machine_id(&mut live_machines.ocw_confirming_machine, id);
        LiveMachines::<T>::put(live_machines);
    }

    // 将machine_id添加到LiveMachines.ocw_confirmed_machine中
    fn add_ocw_confirmed_id(machine_id: MachineId, wallet: Self::AccountId) {
        let mut live_machines = Self::live_machines();

        // 检查wallet是否与用户一致， 如果wallet地址与用户绑定机器的地址不一致，则直接返回
        let machine_info = Self::machines_info(&machine_id);
        if machine_info.machine_owner != wallet {
            return;
        }

        LiveMachine::add_machine_id(&mut live_machines.ocw_confirmed_machine, machine_id);
        LiveMachines::<T>::put(live_machines);
    }
}

// 审查委员会可以执行的操作
impl<T: Config> LCOps for Pallet<T> {
    type MachineId = MachineId;
    type AccountId = T::AccountId;

    // 委员会订阅了一个机器ID
    // 将机器状态从ocw_confirmed_machine改为booked_machine，同时将机器状态改为booked
    fn lc_booked_machine(id: MachineId) {
        let mut live_machines = Self::live_machines();

        LiveMachine::rm_machine_id(&mut live_machines.ocw_confirmed_machine, &id);
        LiveMachine::add_machine_id(&mut live_machines.booked_machine, id.clone());
        LiveMachines::<T>::put(live_machines);

        let mut machine_info = Self::machines_info(&id);
        machine_info.machine_status = MachineStatus::Booked;
        MachinesInfo::<T>::insert(&id, machine_info);
    }

    // 由于委员会没有达成一致，需要重新返回到bonding_machine
    fn lc_revert_booked_machine(id: MachineId) {
        let mut live_machines = Self::live_machines();

        LiveMachine::rm_machine_id(&mut live_machines.booked_machine, &id);
        LiveMachine::add_machine_id(&mut live_machines.ocw_confirmed_machine, id.clone());

        let mut machine_info = Self::machines_info(&id);
        machine_info.machine_status = MachineStatus::Bonding;
        MachinesInfo::<T>::insert(&id, machine_info);
    }

    // 当多个委员会都对机器进行了确认之后，机器的分数被添加上
    fn confirm_machine_grade(who: T::AccountId, machine_id: MachineId, is_confirmed: bool) {
        let mut machine_info = Self::machines_info(&machine_id);
        if machine_info.committee_confirm.contains_key(&who) {
            // TODO: 可以改为返回错误
            return;
        }

        machine_info.committee_confirm.insert(
            who.clone(),
            CommitteeConfirmation {
                committee: who.clone(),
                confirm_time: <frame_system::Module<T>>::block_number(),
                is_confirmed: is_confirmed,
            },
        );

        // 被委员会确认之后，如果未满3个，状态将会改变成bonding_machine, 如果已满3个，则改为waiting_hash状态
        let mut confirmed_committee = vec![];
        for a_committee in &machine_info.committee_confirm {
            confirmed_committee.push(a_committee);
        }

        MachinesInfo::<T>::insert(machine_id.clone(), machine_info.clone());
    }
}

impl<T: Config> Module<T> {
    pub fn get_total_staker_num() -> u64 {
        let temp_account = Self::temp_account();
        return temp_account.len() as u64;
    }

    pub fn get_op_info() -> SysInfo<BalanceOf<T>> {
        SysInfo {
            total_gpu_num: Self::total_gpu_num(),
            total_staker: Self::total_staker(),
            total_calc_points: Self::total_calc_points(),
            total_stake: Self::total_stake(),
        }
    }

    pub fn get_staker_info(account: impl EncodeLike<T::AccountId>) -> StakerInfo<BalanceOf<T>> {
        let staker_info = Self::user_machines(account);

        StakerInfo {
            calc_points: staker_info.total_calc_points,
            gpu_num: staker_info.total_gpu_num,
            total_reward: staker_info.total_reward,
        }
    }

    pub fn get_staker_list(start: u64, end: u64) -> Vec<T::AccountId> {
        let temp_account = Self::temp_account();

        let start = start as usize;
        let mut end = end as usize;

        if start > end || start > temp_account.len() - 1 || temp_account.len() == 0 {
            return Vec::new();
        }

        if end > temp_account.len() - 1 {
            end = temp_account.len() - 1;
        }

        return temp_account[start..end].to_vec();
    }

    pub fn get_staker_identity(account: impl EncodeLike<T::AccountId>) -> Vec<u8> {
        let account_info = <pallet_identity::Module<T>>::identity(account);
        if let None = account_info {
            return Vec::new();
        }
        let account_info = account_info.unwrap();

        match account_info.info.display {
            Data::Raw(out) => return out,
            _ => return Vec::new(),
        }
    }

    // 返回total_page
    pub fn get_staker_list_info(
        cur_page: u64,
        per_page: u64,
    ) -> Vec<StakerListInfo<BalanceOf<T>, T::AccountId>> {
        let temp_account = Self::temp_account();
        let mut out = Vec::new();

        if temp_account.len() == 0 {
            return out;
        }

        let cur_page = cur_page as usize;
        let per_page = per_page as usize;
        let page_start = cur_page * per_page;
        let mut page_end = page_start + per_page;

        if page_start >= temp_account.len() {
            return out;
        }

        if page_end >= temp_account.len() {
            page_end = temp_account.len() - 1;
        }

        for a_account in temp_account[page_start..page_end].into_iter() {
            let staker_info = Self::user_machines(a_account.clone());
            let identity = Self::get_staker_identity(a_account.clone());

            out.push(StakerListInfo {
                staker_name: identity,
                staker_account: a_account.clone(),
                calc_points: staker_info.total_calc_points,
                gpu_num: staker_info.total_gpu_num,
                gpu_rent_rate: 0u64,
                total_reward: staker_info.total_reward,
            })
        }

        return out;
    }
}
