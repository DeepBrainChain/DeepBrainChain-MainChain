#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
    dispatch::DispatchResultWithPostInfo,
    pallet_prelude::*,
    traits::{Currency, Get, LockIdentifier, LockableCurrency, WithdrawReasons},
    IterableStorageMap,
};
use frame_system::pallet_prelude::*;
use online_profile_machine::{LCOps, OCWOps};
use sp_runtime::traits::{CheckedSub, Zero};
use sp_std::{
    collections::btree_map::BTreeMap, collections::btree_set::BTreeSet,
    collections::vec_deque::VecDeque, prelude::*, str,
};

pub mod grade_inflation;
pub mod types;
use types::*;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub const PALLET_LOCK_ID: LockIdentifier = *b"oprofile";
pub const MAX_UNLOCKING_CHUNKS: usize = 32;

// 需要多少个委员会给机器打分
pub const ConfirmGradeLimit: usize = 3;

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MachineInfo<AccountId: Ord, BlockNumber> {
    pub machine_owner: AccountId,
    pub bonding_height: BlockNumber, // 记录机器第一次绑定的时间
    pub bonding_requests: u64,       // 记录机器绑定请求次数，避免绑定错误/无效的机器ID
    pub machine_status: MachineStatus,
    pub ocw_machine_grades: BTreeMap<AccountId, OCWMachineGrade<AccountId, BlockNumber>>, //记录委员会提交的机器打分
    pub ocw_machine_price: u64,           // 记录OCW获取的机器的价格信息
    pub machine_grade: u64,               // 记录根据规则膨胀之后的得分
    pub grade_detail: MachineGradeDetail, //记录OCW获取的机器打分信息
}

// 存储OCW获取到的机器配置各项打分
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug, Copy)]
pub struct MachineGradeDetail {
    pub cpu: u64,
    pub disk: u64,
    pub gpu: u64,
    pub mem: u64,
    pub net: u64,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum MachineStatus {
    Bonding,
    Booked,
    WaitingHash,
    Bonded,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct OCWMachineGrade<AccountId, BlockNumber> {
    pub committee: AccountId,
    pub confirm_time: BlockNumber,
    pub grade: u64,
    pub is_confirmed: bool,
}

impl Default for MachineStatus {
    fn default() -> Self {
        MachineStatus::Bonding
    }
}

// 只保存正常声明周期的Machine,删除掉的/绑定失败的不保存在该变量中
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct LiveMachine {
    pub bonding_machine: Vec<MachineId>,
    pub booked_machine: Vec<MachineId>,
    pub waiting_hash: Vec<MachineId>,
    pub bonded_machine: Vec<MachineId>,
}

type BalanceOf<T> =
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

// #[rustfmt::skip]
#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + random_num::Config + dbc_price_ocw::Config {
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

    #[pallet::storage]
    pub(super) type ProfitReleaseDuration<T: Config> =
        StorageValue<_, u64, ValueQuery, ProfitReleaseDurationDefault<T>>;

    // 存储机器的最小质押量，单位DBC
    #[pallet::storage]
    #[pallet::getter(fn min_stake)]
    pub(super) type MinStake<T> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    // 机器的详细信息,只有当所有奖励领取完才能删除该变量?
    /// MachineDetail
    /// TODO: MachineDetail变为MachineInfo
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
    pub(super) type UserMachines<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, Vec<MachineId>, ValueQuery>;

    // 存储活跃的机器
    #[pallet::storage]
    #[pallet::getter(fn live_machines)]
    pub type LiveMachines<T: Config> = StorageValue<_, LiveMachine, ValueQuery>;

    // // 用户提交绑定请求
    // #[pallet::storage]
    // #[pallet::getter(fn bonding_queue)]
    // pub type BondingMachine<T> = StorageMap<
    //     _,
    //     Blake2_128Concat,
    //     MachineId,
    //     BondingPair<<T as frame_system::Config>::AccountId>,
    //     ValueQuery,
    // >;

    // #[pallet::storage]
    // #[pallet::getter(fn booking_queue)]
    // pub type BookingMachine<T> = StorageMap<
    //     _,
    //     Blake2_128Concat,
    //     MachineId,
    //     BookingItem<<T as frame_system::Config>::BlockNumber>, // TODO: 修改类型 需要有height, who, machineid
    // >;

    // #[pallet::storage]
    // #[pallet::getter(fn booked_queue)]
    // pub type BookedMachine<T> = StorageMap<_, Blake2_128Concat, MachineId, u64, ValueQuery>; //TODO: 修改类型，保存已经被委员会预订的机器

    // /// Machine has been bonded
    // /// 记录成功绑定的机器ID
    // #[pallet::storage]
    // #[pallet::getter(fn bonded_machine)]
    // pub type BondedMachine<T> = StorageMap<_, Blake2_128Concat, MachineId, (), ValueQuery>;

    // TODO: 这里遍历用户绑定的所有机器记录即可
    // // 记录用户绑定的机器ID列表
    // #[pallet::storage]
    // #[pallet::getter(fn user_bonded_machine)]
    // pub(super) type UserBondedMachine<T: Config> = StorageMap<
    //     _,
    //     Blake2_128Concat,
    //     T::AccountId,
    //     Vec<MachineId>,
    //     ValueQuery,
    // >;

    // // 记录用户成功绑定的机器列表，用以查询以及计算奖励膨胀系数
    // #[pallet::storage]
    // #[pallet::getter(fn user_bonded_succeed)]
    // pub(super) type UserBondedSucceed<T: Config> = StorageMap<
    //     _,
    //     Blake2_128Concat,
    //     T::AccountId,
    //     Vec<MachineId>,
    //     ValueQuery,
    // >;

    // TODO: 这里存储到机器信息里面
    // // 存储机器的当前得分，当有影响该机器得分的因素发生是时，改变该变量
    // // 精度：10000
    // #[pallet::storage]
    // #[pallet::getter(fn bonded_machines_grade)]
    // pub(super) type BondedMachineGrade<T: Config> = StorageMap<
    //     _,
    //     Blake2_128Concat,
    //     MachineId,
    //     u64,
    //     ValueQuery
    // >;

    // TODO: 这里只要抽象成一个函数即可
    // // 如果账户绑定机器越多，分数膨胀将会越大
    // // 该数值精度为10000
    // // 该数值应该与机器数量呈正相关，以避免有极值导致用户愿意拆分机器数量到不同账户
    // #[pallet::storage]
    // #[pallet::getter(fn user_bonded_inflation)]
    // pub(super) type UserBondedInflation<T: Config> = StorageMap<
    //     _,
    //     Blake2_128Concat,
    //     T::AccountId,
    //     u64,
    //     ValueQuery,
    // >;

    // TODO: 可以直接更新到machinesInfo因为startEra会创建交易快照
    // 当机器已经被成功绑定，则出现需要更新机器的奖励膨胀系数时，改变该变量
    // 可能的场景有：
    //  1. 新的一台机器被成功绑定
    //  2. 一台机器被移除绑定
    //  3. 一台被成功绑定的机器的质押数量发生变化
    // #[pallet::storage]
    // #[pallet::getter(fn grade_updating)]
    // pub(super) type GradeUpdating<T: Config> =
    //     StorageMap<_, Blake2_128Concat, T::AccountId, VecDeque<MachineId>, ValueQuery>;

    // TODO: 移动到一个变量中即可
    // // 存储所有绑定的机器，用于OCW轮询验证是否在线
    // #[pallet::storage]
    // #[pallet::getter(fn staking_machine)]
    // pub(super) type StakingMachine<T> = StorageMap<_, Blake2_128Concat, MachineId, Vec<bool>, ValueQuery>;

    // TODO: 存储到机器的所有信息的变量中
    // // 存储ocw获取的机器打分信息
    // // 与委员会的确认信息
    // #[pallet::storage]
    // #[pallet::getter(fn ocw_machine_grades)]
    // pub type OCWMachineGrades<T: Config> = StorageMap<
    //     _,
    //     Blake2_128Concat,
    //     MachineId,
    //     ConfirmedMachine<T::AccountId, T::BlockNumber>,
    //     ValueQuery,
    // >;

    // 存储到机器的所有信息中
    // // 存储ocw获取的机器估价信息
    // #[pallet::storage]
    // #[pallet::getter(fn ocw_machine_price)]
    // pub type OCWMachinePrice<T> = StorageMap<_, Blake2_128Concat, MachineId, u64, ValueQuery>;

    /// Map from all (unlocked) "controller" accounts to the info regarding the staking.
    #[pallet::storage]
    #[pallet::getter(fn ledger)]
    pub(super) type Ledger<T> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        <T as frame_system::Config>::AccountId,
        Blake2_128Concat,
        MachineId,
        Option<StakingLedger<<T as frame_system::Config>::AccountId, BalanceOf<T>>>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn eras_reward_points)]
    pub(super) type ErasRewardGrades<T> = StorageMap<
        _,
        Blake2_128Concat,
        EraIndex,
        EraRewardGrades<<T as frame_system::Config>::AccountId>,
        ValueQuery,
    >;

    // 奖励数量：第一个月为1亿，之后每个月为3300万
    // 2年10个月之后，奖励数量减半，之后再五年，奖励减半
    #[pallet::storage]
    #[pallet::getter(fn reward_per_year)]
    pub(super) type RewardPerYear<T> = StorageValue<_, BalanceOf<T>>;

    // 等于RewardPerYear * (era_duration / year_duration)
    #[pallet::storage]
    #[pallet::getter(fn eras_validator_reward)]
    pub(super) type ErasValidatorReward<T> =
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
        fn set_min_stake(
            origin: OriginFor<T>,
            new_min_stake: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            MinStake::<T>::put(new_min_stake);
            Ok(().into())
        }

        // TODO: 如果验证结果发现，绑定者与机器钱包地址不一致，则进行惩罚
        // TODO: era结束时重新计算得分, 如果有会影响得分的改变，放到列表中，等era结束进行计算

        // 将machine_id添加到绑定队列
        /// Bonding machine only remember caller-machine_id pair.
        /// OCW will check it and record machine info.
        #[pallet::weight(10000)]
        fn bond_machine(
            origin: OriginFor<T>,
            machine_id: MachineId,
            bond_amount: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;

            // 资金检查
            ensure!(
                bond_amount >= MinStake::<T>::get(),
                Error::<T>::StakeNotEnough
            );
            ensure!(
                <T as Config>::Currency::free_balance(&controller) > bond_amount,
                Error::<T>::BalanceNotEnough
            );
            ensure!(
                bond_amount >= T::Currency::minimum_balance(),
                Error::<T>::InsufficientValue
            );

            // TODO: 更改函数返回类型
            ensure!(
                !Self::machine_id_exist(&machine_id),
                Error::<T>::MachineIdExist
            );

            // TODO: 增加修改三个变量的Event
            // 添加到用户的机器列表
            let mut user_machines = Self::user_machines(&controller);
            if let Err(index) = user_machines.binary_search(&machine_id) {
                user_machines.insert(index, machine_id.clone());
            } else {
                return Err(Error::<T>::MachineInUserBonded.into());
            }

            // 添加到LiveMachine
            Self::add_bonding_machine(machine_id.clone());

            // 添加到MachinesInfo
            let machine_info = MachineInfo {
                machine_owner: controller.clone(),
                bonding_height: <frame_system::Module<T>>::block_number(),
                ..Default::default()
            };
            MachinesInfo::<T>::insert(&machine_id, machine_info);

            // 直接初始化Ledger, 如果绑定失败，则调用unbond方法，进行自动解邦.
            let current_era = <random_num::Module<T>>::current_era();
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

        #[pallet::weight(10000)]
        fn bond_extra(
            origin: OriginFor<T>,
            machine_id: MachineId,
            max_additional: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;

            let mut ledger =
                Self::ledger(&controller, &machine_id).ok_or(Error::<T>::LedgerNotFound)?;
            let user_balance = T::Currency::free_balance(&controller);

            if let Some(extra) = user_balance.checked_sub(&ledger.total) {
                let extra = extra.min(max_additional);
                ledger.total += extra;
                ledger.active += extra;

                ensure!(
                    ledger.active >= T::Currency::minimum_balance(),
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

        #[pallet::weight(10000)]
        fn unbond(
            origin: OriginFor<T>,
            machine_id: MachineId,
            amount: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;

            let mut ledger =
                Self::ledger(&controller, &machine_id).ok_or(Error::<T>::LedgerNotFound)?;
            ensure!(
                ledger.unlocking.len() < crate::MAX_UNLOCKING_CHUNKS,
                Error::<T>::NoMoreChunks
            );

            let mut value = amount.min(ledger.active);
            if !value.is_zero() {
                ledger.active -= value;

                if ledger.active < <T as Config>::Currency::minimum_balance() {
                    value += ledger.active;
                    ledger.active = Zero::zero();
                }

                let era = <random_num::Module<T>>::current_era() + T::BondingDuration::get();
                ledger.unlocking.push(UnlockChunk { value, era });

                Self::update_ledger(&controller, &machine_id, &ledger);
                Self::deposit_event(Event::RemoveBonded(controller, machine_id, value));
            }

            Ok(().into())
        }

        #[pallet::weight(10000)]
        fn withdraw_unbonded(
            origin: OriginFor<T>,
            machine_id: MachineId,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;

            let mut ledger =
                Self::ledger(&controller, &machine_id).ok_or(Error::<T>::LedgerNotFound)?;

            let old_total = ledger.total;
            let current_era = <random_num::Module<T>>::current_era();
            ledger = ledger.consolidate_unlock(current_era);

            if ledger.unlocking.is_empty() && ledger.active <= T::Currency::minimum_balance() {
                // 清除ledger相关存储
                T::Currency::remove_lock(crate::PALLET_LOCK_ID, &controller);
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

        // #[pallet::weight(10000)]
        // pub fn payout_rewards(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
        //     let who = ensure_signed(origin)?;
        //     let current_era = Self::current_era();
        //     // Self::do_payout_stakers(who, era);

        //     // let reward_to_payout = UserReleasedReward::<T>::get(&user);
        //     // let _ = <T as Config>::Currency::deposit_into_existing(&user, reward_to_payout).ok();

        //     // <UserPayoutEraIndex<T>>::insert(user, current_era);
        //     Ok(().into())
        // }

        // TODO: 委员会通过lease-committee设置机器价格
        // #[pallet::weight(0)]
        // pub fn set_machine_price(
        //     origin: OriginFor<T>,
        //     machine_id: MachineId,
        //     machine_price: u64,
        // ) -> DispatchResultWithPostInfo {
        //     let _ = ensure_root(origin)?;

        //     if !MachineDetail::<T>::contains_key(&machine_id) {
        //         MachineDetail::<T>::insert(
        //             &machine_id,
        //             MachineMeta {
        //                 machine_price: machine_price,
        //                 machine_grade: 0,
        //                 committee_confirm: vec![],
        //             },
        //         );
        //         return Ok(().into());
        //     }

        //     let mut machine_detail = MachineDetail::<T>::get(&machine_id);
        //     machine_detail.machine_price = machine_price;

        //     MachineDetail::<T>::insert(&machine_id, machine_detail);
        //     Ok(().into())
        // }
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

    // 如果在booked_machine中，则从中删除
    // 添加到bonding_machine中
    fn add_bonding_machine(id: MachineId) {
        let mut live_machines = Self::live_machines();
        if let Ok(index) = live_machines.booked_machine.binary_search(&id) {
            live_machines.booked_machine.remove(index);
        }
        if let Err(index) = live_machines.bonding_machine.binary_search(&id) {
            live_machines.bonding_machine.insert(index, id)
        }
        LiveMachines::<T>::put(live_machines);
    }

    // 如果存在于bonding_machine中，则从中删掉
    // 添加到booked_machine中
    pub fn add_booked_machine(id: MachineId) {
        let mut live_machines = Self::live_machines();
        if let Ok(index) = live_machines.bonding_machine.binary_search(&id) {
            live_machines.bonding_machine.remove(index);
        }
        if let Err(index) = live_machines.booked_machine.binary_search(&id) {
            live_machines.booked_machine.insert(index, id);
        }
        LiveMachines::<T>::put(live_machines);
    }

    // 如果存在于booked_machine中，则从中删除
    // 添加到waiting_hash中
    fn add_waiting_hash(id: MachineId) {
        let mut live_machines = Self::live_machines();
        if let Ok(index) = live_machines.booked_machine.binary_search(&id) {
            live_machines.booked_machine.remove(index);
        }
        if let Err(index) = live_machines.waiting_hash.binary_search(&id) {
            live_machines.waiting_hash.insert(index, id);
        }
        LiveMachines::<T>::put(live_machines);
    }

    // 如果存在于waiting_hash中，则从中删除
    // 添加到bonded_machine中
    fn add_bonded_machine(id: MachineId) {
        let mut live_machines = Self::live_machines();
        if let Ok(index) = live_machines.waiting_hash.binary_search(&id) {
            live_machines.waiting_hash.remove(index);
        }
        if let Err(index) = live_machines.bonded_machine.binary_search(&id) {
            live_machines.bonded_machine.insert(index, id);
        }
        LiveMachines::<T>::put(live_machines);
    }

    // TODO: 改成返回错误或者Ok
    // 查询是否存在于活跃的机器列表中
    fn machine_id_exist(id: &MachineId) -> bool {
        let live_machines = Self::live_machines();

        if let Ok(_) = live_machines.bonding_machine.binary_search(id) {
            return true;
        }
        if let Ok(_) = live_machines.booked_machine.binary_search(id) {
            return true;
        }
        if let Ok(_) = live_machines.waiting_hash.binary_search(id) {
            return true;
        }
        if let Ok(_) = live_machines.bonded_machine.binary_search(id) {
            return true;
        }
        return false;
    }

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
        // grade_inflation::compute_stake_grades(machine_price, staked_in, machine_grade)
    }

    // 更新用户的质押的ledger
    fn update_ledger(
        controller: &T::AccountId,
        machine_id: &MachineId,
        ledger: &StakingLedger<T::AccountId, BalanceOf<T>>,
    ) {
        T::Currency::set_lock(
            PALLET_LOCK_ID,
            &ledger.stash,
            ledger.total,
            WithdrawReasons::all(),
        );
        Ledger::<T>::insert(controller, machine_id, Some(ledger));
    }
}

impl<T: Config> OCWOps for Pallet<T> {
    // type AccountId = T::AccountId;
    // type BlockNumber = T::BlockNumber;
    type MachineId = MachineId;
    type MachineInfo = MachineInfo<T::AccountId, T::BlockNumber>;

    fn update_machine_info(id: &MachineId, machine_info: Self::MachineInfo) {
        MachinesInfo::<T>::insert(id, machine_info);
    }
}

impl<T: Config> LCOps for Pallet<T> {
    type MachineId = MachineId;
    type AccountId = T::AccountId;

    // TODO: 从OCW获取机器打分可能会失败，改为Option类型
    fn confirm_machine_grade(who: T::AccountId, machine_id: MachineId, is_confirmed: bool) {
        let mut machine_info = Self::machines_info(&machine_id);
        if machine_info.ocw_machine_grades.contains_key(&who) {
            // TODO: 可以改为返回错误
            return;
        }

        machine_info.ocw_machine_grades.insert(
            who.clone(),
            OCWMachineGrade {
                committee: who,
                confirm_time: <frame_system::Module<T>>::block_number(),
                grade: 0,
                is_confirmed: is_confirmed,
            },
        );

        MachinesInfo::<T>::insert(machine_id.clone(), machine_info.clone());

        // 被委员会确认之后，如果未满3个，状态将会改变成bonding_machine, 如果已满3个，则改为waiting_hash状态
        let mut confirmed_committee = vec![];
        for a_committee in machine_info.ocw_machine_grades {
            confirmed_committee.push(a_committee);
        }

        if confirmed_committee.len() == ConfirmGradeLimit {
            Self::add_waiting_hash(machine_id);
        } else {
            Self::add_bonding_machine(machine_id);
        }
    }

    fn lc_add_booked_machine(id: MachineId) {
        Self::add_booked_machine(id);
    }
}
