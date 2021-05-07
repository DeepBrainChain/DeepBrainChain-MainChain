#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
    dispatch::DispatchResultWithPostInfo,
    pallet_prelude::*,
    traits::{Currency, Get, LockIdentifier, LockableCurrency, WithdrawReasons},
};
use frame_system::pallet_prelude::*;
use online_profile_machine::{LCOps, OCWOps};
use sp_runtime::traits::{CheckedSub, Zero};
use sp_runtime::SaturatedConversion;
use sp_std::{
    collections::btree_map::BTreeMap, collections::btree_set::BTreeSet,
    collections::vec_deque::VecDeque, prelude::*, str,
};

pub mod grade_inflation;
pub mod machine_info;
pub mod types;
use types::*;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub const PALLET_LOCK_ID: LockIdentifier = *b"oprofile";
pub const MAX_UNLOCKING_CHUNKS: usize = 32;

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MachineInfo<AccountId: Ord, BlockNumber> {
    pub machine_owner: AccountId,
    pub bonding_height: BlockNumber, // 记录机器第一次绑定的时间, TODO: 改为current_slot
    pub machine_status: MachineStatus,
    pub ocw_machine_info: machine_info::OCWMachineInfo,
    pub machine_grade: u64,
    pub committee_confirm: BTreeMap<AccountId, CommitteeConfirmation<AccountId, BlockNumber>>, //记录委员会提交的机器打分
    pub reward_committee: Vec<AccountId>, // 列表中的委员将分得用户奖励
    pub reward_deadline: BlockNumber, // 列表中委员分得奖励结束时间 , TODO: 绑定时间改为current_slot比较好
}

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
    // 当machine的确认hash未满时, 委员会从cow_confirmed_machine中读取可以审查的机器ID,
    // 添加确认信息之后，状态变为`ocw_confirmed_machine`，这时可以继续抢单.但已经打分过的委员不能抢该单
    pub booked_machine: Vec<MachineId>,

    pub ocw_confirmed_machine: Vec<MachineId>, // OCW从bonding_machine中读取机器ID，确认之后，添加到本字段
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

    fn rm_machine_id(a_field: &mut Vec<MachineId>, machine_id: MachineId) {
        if let Ok(index) = a_field.binary_search(&machine_id) {
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
    pub trait Config: frame_system::Config + dbc_price_ocw::Config + random_num::Config {
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
    pub(super) type ProfitReleaseDuration<T: Config> =
        StorageValue<_, u64, ValueQuery, ProfitReleaseDurationDefault<T>>;

    #[pallet::type_value]
    pub fn CommitteeLimitDefault<T: Config>() -> u32 {
        3
    }

    #[pallet::storage]
    #[pallet::getter(fn committee_limit)]
    pub type CommitteeLimit<T: Config> = StorageValue<_, u32, ValueQuery, CommitteeLimitDefault<T>>;

    // 存储机器的最小质押量，单位DBC
    #[pallet::storage]
    #[pallet::getter(fn stake_per_gpu)]
    pub(super) type StakePerGPU<T> = StorageValue<_, BalanceOf<T>, ValueQuery>;

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
    pub(super) type UserMachines<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, Vec<MachineId>, ValueQuery>;

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
    #[pallet::getter(fn eras_reward_points)]
    pub(super) type ErasRewardGrades<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        EraIndex,
        EraRewardGrades<T::AccountId>,
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
            // 从ocw读取价格
            Self::update_min_stake_dbc();

            // if (block_number.saturated_into::<u64>() + 1) / T::BlockPerEra::get() as u64 == 0 {
            //     Self::end_era()
            // }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(0)]
        pub fn set_reward_start_height(origin: OriginFor<T>, reward_start_height: T::BlockNumber) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            RewardStartHeight::<T>::put(reward_start_height);
            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn set_committee_limit(origin: OriginFor<T>, limit: u32) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            CommitteeLimit::<T>::put(limit);
            Ok(().into())
        }

        // 设置单卡质押数量
        #[pallet::weight(0)]
        pub fn set_gpu_stake(origin: OriginFor<T>, stake_per_gpu: BalanceOf<T>) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            StakePerGPU::<T>::put(stake_per_gpu);
            Ok(().into())
        }

        // TODO: 如果验证结果发现，绑定者与机器钱包地址不一致，则进行惩罚
        // TODO: era结束时重新计算得分, 如果有会影响得分的改变，放到列表中，等era结束进行计算

        // 将machine_id添加到绑定队列,
        /// Bonding machine only remember caller-machine_id pair.
        /// OCW will check it and record machine info.
        #[pallet::weight(10000)]
        pub fn bond_machine(origin: OriginFor<T>, machine_id: MachineId, gpu_num: u32) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;

            let bond_amount = Self::stake_per_gpu() * gpu_num.into(); // TODO: 改为checked_mul

            // 资金检查
            // ensure!(bond_amount >= StakePerGPU::<T>::get(), Error::<T>::StakeNotEnough);
            ensure!(<T as Config>::Currency::free_balance(&controller) > bond_amount, Error::<T>::BalanceNotEnough);
            ensure!(bond_amount >= <T as pallet::Config>::Currency::minimum_balance(), Error::<T>::InsufficientValue);
            // 确保机器还没有被绑定过
            let mut live_machines = Self::live_machines();
            ensure!(!live_machines.machine_id_exist(&machine_id), Error::<T>::MachineIdExist);

            // 添加到用户的机器列表
            let mut user_machines = Self::user_machines(&controller);
            if let Err(index) = user_machines.binary_search(&machine_id) {
                user_machines.insert(index, machine_id.clone());
                UserMachines::<T>::insert(&controller, user_machines);
            } else {
                return Err(Error::<T>::MachineInUserBonded.into());
            }

            // 添加到LiveMachine的bonding_machine字段
            LiveMachine::add_machine_id(&mut live_machines.bonding_machine, machine_id.clone());
            LiveMachines::<T>::put(live_machines);

            // 初始化MachineInfo, 并添加到MachinesInfo
            let machine_info = MachineInfo {
                machine_owner: controller.clone(),
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
                total: bond_amount,
                active: bond_amount,
                unlocking: vec![],
                claimed_rewards: (last_reward_era..current_era).collect(),
                released_rewards: 0u32.into(),
                upcoming_rewards: VecDeque::new(),
            };

            Self::update_ledger(&controller, &machine_id, &item);

            Self::deposit_event(Event::BondMachine(controller, machine_id, bond_amount));

            Ok(().into())
        }

        // 当用户被罚款后，需要补充质押金额
        #[pallet::weight(10000)]
        fn fulfill_bond(origin: OriginFor<T>, machine_id: MachineId) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;

            //  max_additional: BalanceOf<T>
            let bond_extra: BalanceOf<T> = 0u32.into();

            let mut ledger = Self::ledger(&controller, &machine_id).ok_or(Error::<T>::LedgerNotFound)?;
            let user_balance = <T as pallet::Config>::Currency::free_balance(&controller);

            if let Some(extra) = user_balance.checked_sub(&ledger.total) {
                let extra = extra.min(bond_extra);
                ledger.total += extra;
                ledger.active += extra;

                ensure!(
                    ledger.active >= <T as pallet::Config>::Currency::minimum_balance(),
                    Error::<T>::InsufficientValue
                );

                Self::deposit_event(Event::AddBonded(
                    controller.clone(),
                    machine_id.clone(),
                    extra,
                ));
                Self::update_ledger(&controller, &machine_id, &ledger);
            }

            Ok(().into())
        }

        // TODO: 确定退出时机
        // 当用户想要机器从中退出时，可以调用unbond，来取出质押的金额
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

                Self::update_ledger(&controller, &machine_id, &ledger);
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
                Self::update_ledger(&controller, &machine_id, &ledger);
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

        #[pallet::weight(10000)]
        pub fn payout_rewards(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            // let current_era = Self::current_era();
            // Self::do_payout_stakers(who, era);

            // let reward_to_payout = UserReleasedReward::<T>::get(&user);
            // let _ = <T as Config>::Currency::deposit_into_existing(&user, reward_to_payout).ok();

            // <UserPayoutEraIndex<T>>::insert(user, current_era);
            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn cancle_slash(origin: OriginFor<T>, machine_id: MachineId) -> DispatchResultWithPostInfo {
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
    }
}

impl<T: Config> Pallet<T> {
    // fn do_payout_stakers(who: T::AccountId, era: EraIndex) -> DispatchResult {
    //     let current_era = Self::current_era();
    //     ensure!(era <= current_era, Error::<T>::InvalidEraToReward);
    //     let history_depth = Self::history_depth();
    //     ensure!(
    //         era >= current_era.saturating_sub(history_depth),
    //         Error::<T>::InvalidEraToReward
    //     );

    //     let era_payout =
    //         <ErasValidatorReward<T>>::get(&era).ok_or_else(|| Error::<T>::InvalidEraToReward)?;

    //     Ok(())
    // }

    // fn confirmed_committee(id: MachineId) -> BTreeSet<AccountId> {
    //     let machines_info = Self::machines_info(&id);

    // }

    // TODO: 在这四个函数中，更新machine的状态

    // Update min_stake_dbc every block end
    fn update_min_stake_dbc() {
        // TODO: 1. 获取DBC价格
        let dbc_price = dbc_price_ocw::Pallet::<T>::avg_price;

        // TODO: 2. 计算所需DBC

        // TODO: 3. 更新min_stake_dbc变量
    }

    // 为机器增加得分奖励
    pub fn reward_by_ids(
        era_index: u32,
        validators_points: impl IntoIterator<Item = (T::AccountId, u32)>,
    ) {
        <ErasRewardGrades<T>>::mutate(era_index, |era_rewards| {
            for (validator, grades) in validators_points.into_iter() {
                *era_rewards.individual.entry(validator).or_default() += grades;
                era_rewards.total += grades;
            }
        });
    }

    // 影响机器得分因素：
    // 1. 基础得分(从API获取)
    // 2. 质押数量
    // 3. 用户总绑定机器个数
    // 待添加: 4. 机器在线时长奖励
    fn calc_machine_grade(machine_id: MachineId) -> u64 {
        // TODO: 如何获得机器基本得分情况？ 应该由OCW写入到本模块变量中

        // 1. 查询机器拥有者

        // 2. 查询机器质押数量

        0
    }

    // 更新以下信息:
    // [机器质押信息] TODO: 质押一定数量之后再解邦
    // [机器质押代币总数量] (这就是为什么需要14天解绑，保证今天解绑不会影响今天总质押)
    // [机器总打分信息]
    // [机器分别的打分信息]

    // 可能在一个Era中再次更新的信息:
    // [机器打分信息]: 如果有减少，则立即更新，如果有增加，则下一个时间更新
    // [机器总打分信息]: 如果某一个机器打分减少，则总打分信息也会变化
    fn start_era() {
        // TODO: 在start_era 的时候，更新打分信息,记录质押信息,可以加一个全局锁，将这段函数放在OCW中完成

        let current_era = <random_num::Module<T>>::current_era();
        // let bonded_machine_id = Self::bonded_machine_id();

        // for a_machine_id in bonded_machine_id.iter() {
        //     let machine_grade = 1;
        //     // <ErasMachineGrades<T>>::insert(&current_era, "machine_id", machine_grade);
        // }

        // let a_machine_grade = 1;

        // TODO: 清理未收集的数据

        // TODO: 触发惩罚
    }

    fn end_era() {
        // TODO: 参考staking模块的end_era
        // grade_inflation::compute_stake_grades(machine_price, staked_in, machine_grade)
    }

    fn add_daily_reward(controller: T::AccountId, machine_id: MachineId, amount: BalanceOf<T>) {
        let mut ledger = Self::ledger(&controller, &machine_id);
        // TODO: 将amount 的25%直接添加到用户的资金
        // TODO：75%添加到用户的剩下150天的余额中
    }

    // 更新用户的质押的ledger
    fn update_ledger(
        controller: &T::AccountId,
        machine_id: &MachineId,
        ledger: &StakingLedger<T::AccountId, BalanceOf<T>>,
    ) {
        <T as pallet::Config>::Currency::set_lock(
            PALLET_LOCK_ID,
            &ledger.stash,
            ledger.total,
            WithdrawReasons::all(),
        );
        Ledger::<T>::insert(controller, machine_id, Some(ledger));
    }
}

// online-profile-ocw可以执行的操作
impl<T: Config> OCWOps for Pallet<T> {
    type MachineId = MachineId;
    type MachineInfo = MachineInfo<T::AccountId, T::BlockNumber>;

    fn rm_bonding_id(id: MachineId) {
        let mut live_machines = Self::live_machines();
        LiveMachine::rm_machine_id(&mut live_machines.bonding_machine, id);
        LiveMachines::<T>::put(live_machines);
    }

    fn add_ocw_confirmed_id(id: MachineId) {
        let mut live_machines = Self::live_machines();
        LiveMachine::add_machine_id(&mut live_machines.ocw_confirmed_machine, id);
        LiveMachines::<T>::put(live_machines);
    }

    fn update_machine_info(id: &MachineId, machine_info: Self::MachineInfo) {
        MachinesInfo::<T>::insert(id, machine_info);
    }
}

// 审查委员会可以执行的操作
impl<T: Config> LCOps for Pallet<T> {
    type MachineId = MachineId;
    type AccountId = T::AccountId;

    fn book_machine(id: MachineId) {}
    // fn submit_confirm_hash(who: T::AccountId, machine_id: MachineId, raw_hash: MachineId) {}
    // fn submit_raw_confirmation(
    //     who: T::AccountId,
    //     machine_id: MachineId,
    //     raw_confirmation: MachineId,
    // ) {
    // }

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

        if confirmed_committee.len() as u32 == Self::committee_limit() {
            // 检查是否通过
            // TODO: 检查是否全部同意，并更改机器状态
        }

        MachinesInfo::<T>::insert(machine_id.clone(), machine_info.clone());
    }

    // 委员会订阅了一个机器ID
    fn lc_add_booked_machine(id: MachineId) {
        let mut live_machines = Self::live_machines();

        LiveMachine::rm_machine_id(&mut live_machines.booked_machine, id.clone());
        LiveMachine::add_machine_id(&mut live_machines.booked_machine, id.clone());
        LiveMachines::<T>::put(live_machines);

        let mut machine_info = Self::machines_info(&id);
        machine_info.machine_status = MachineStatus::Booked;
        MachinesInfo::<T>::insert(&id, machine_info);
    }
}
