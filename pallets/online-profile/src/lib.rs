#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
    dispatch::DispatchResultWithPostInfo,
    pallet_prelude::*,
    traits::{Currency, Get, LockIdentifier, LockableCurrency, WithdrawReasons},
    IterableStorageMap,
};
use frame_system::pallet_prelude::*;
use online_profile_machine::{LCOps, OLProof, OPOps};
use sp_runtime::traits::{CheckedSub, Zero};
use sp_std::{collections::btree_set::BTreeSet, collections::vec_deque::VecDeque, prelude::*, str};

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

type BalanceOf<T> =
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[rustfmt::skip]
#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + random_num::Config + dbc_price_ocw::Config  {
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
    pub(super) type HistoryDepth<T: Config> = StorageValue<_, u32, ValueQuery, HistoryDepthDefault<T>>;

    // 用户线性释放的天数:
    // 25%收益当天释放；75%在150天线性释放
    #[pallet::type_value]
    pub(super) fn ProfitReleaseDurationDefault<T: Config>() -> u64 {
        150
    }

    // 单位美分
    #[pallet::storage]
    #[pallet::getter(fn min_stake_cent)]
    pub(super) type MinStakeCent<T> = StorageValue<_, u64, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn min_stake_dbc)]
    pub(super) type MinStakeDBC<T> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    #[pallet::storage]
    pub(super) type ProfitReleaseDuration<T: Config> = StorageValue<_, u64, ValueQuery, ProfitReleaseDurationDefault<T>>;

    /// MachineDetail
    #[pallet::storage]
    #[pallet::getter(fn machine_detail)]
    pub type MachineDetail<T> = StorageMap<
        _,
        Blake2_128Concat,
        MachineId,
        MachineMeta<<T as frame_system::Config>::AccountId>,
        ValueQuery,
    >;

    // 用户提交绑定请求
    #[pallet::storage]
    #[pallet::getter(fn bonding_queue)]
    pub type BondingMachine<T> = StorageMap<
        _,
        Blake2_128Concat,
        MachineId,
        BondingPair<<T as frame_system::Config>::AccountId>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn booking_queue)]
    pub type BookingMachine<T> = StorageMap<
        _,
        Blake2_128Concat,
        MachineId,
        BookingItem<<T as frame_system::Config>::BlockNumber>, // TODO: 修改类型 需要有height, who, machineid
    >;

    #[pallet::storage]
    #[pallet::getter(fn booked_queue)]
    pub type BookedMachine<T> = StorageMap<_, Blake2_128Concat, MachineId, u64, ValueQuery>; //TODO: 修改类型，保存已经被委员会预订的机器

    /// Machine has been bonded
    #[pallet::storage]
    #[pallet::getter(fn bonded_machine)]
    pub type BondedMachine<T> = StorageMap<_, Blake2_128Concat, MachineId, (), ValueQuery>;

    // 记录用户绑定的机器ID列表
    #[pallet::storage]
    #[pallet::getter(fn user_bonded_machine)]
    pub(super) type UserBondedMachine<T> = StorageMap<
        _,
        Blake2_128Concat,
        <T as frame_system::Config>::AccountId,
        Vec<MachineId>,
        ValueQuery,
    >;

    // 存储所有绑定的机器，用于OCW轮询验证是否在线
    #[pallet::storage]
    #[pallet::getter(fn staking_machine)]
    pub(super) type StakingMachine<T> = StorageMap<_, Blake2_128Concat, MachineId, Vec<bool>, ValueQuery>;

    // 存储ocw获取的机器打分信息
    // 与委员会的确认信息
    #[pallet::storage]
    #[pallet::getter(fn ocw_machine_grades)]
    pub type OCWMachineGrades<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        MachineId,
        ConfirmedMachine<T::AccountId, T::BlockNumber>,
        ValueQuery,
    >;

    // 存储ocw获取的机器估价信息
    #[pallet::storage]
    #[pallet::getter(fn ocw_machine_price)]
    pub type OCWMachinePrice<T> = StorageMap<_, Blake2_128Concat, MachineId, u64, ValueQuery>;

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

    // // Reward per year
    // // TODO：奖励是按照一定规则发放的。
    // #[pallet::storage]
    // #[pallet::getter(fn reward_per_year)]
    // pub(super) type RewardPerYear<T> = StorageValue<_, BalanceOf<T>>;

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
        fn set_min_stake(origin: OriginFor<T>, new_min_stake: u64) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            MinStakeCent::<T>::put(new_min_stake);

            Ok(().into())
        }

        // 将machine_id添加到绑定队列
        /// Bonding machine only remember caller-machine_id pair.
        /// OCW will check it and record machine info.
        #[pallet::weight(10000)]
        fn bond_machine(origin: OriginFor<T>, machine_id: MachineId, bond_amount: BalanceOf<T>) -> DispatchResultWithPostInfo {
            let caller = ensure_signed(origin)?;

            // 确保 BondingMachine 不包含该 machine_id
            ensure!(!BondingMachine::<T>::contains_key(&machine_id), Error::<T>::MachineInBondingMachine);
            ensure!(!BookingMachine::<T>::contains_key(&machine_id), Error::<T>::MachineInBookingMachine);
            ensure!(!BookedMachine::<T>::contains_key(&machine_id), Error::<T>::MachineInBookedMachine);
            // 该machine_id还未被绑定
            ensure!(!BondedMachine::<T>::contains_key(&machine_id), Error::<T>::MachineHasBonded);

            BondingMachine::<T>::insert(
                machine_id.clone(),
                BondingPair {
                    account_id: caller,
                    machine_id: machine_id,
                    request_count: 0,
                },
            );

            Ok(().into())
        }

        #[pallet::weight(10000)]
        fn add_bonded_token(
            origin: OriginFor<T>,
            machine_id: MachineId,
            bond_amount: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            // 检查余额
            ensure!(<T as Config>::Currency::free_balance(&who) > bond_amount, Error::<T>::BalanceNotEnough);
            // 检查超过最小交易金额
            ensure!(bond_amount >= T::Currency::minimum_balance(), Error::<T>::InsufficientValue);

            // 检查用户已绑定了该机器
            let user_bonded_machine = UserBondedMachine::<T>::get(&who);
            if let Err(_) = user_bonded_machine.binary_search(&machine_id) {
                return Err(Error::<T>::MachineIdNotBonded.into());
            };

            let current_era = <random_num::Module<T>>::current_era();
            let history_depth = Self::history_depth(); // TODO: add this
            let last_reward_era = current_era.saturating_sub(history_depth);

            let user_balance = T::Currency::free_balance(&who);
            let bond_amount = bond_amount.min(user_balance);

            Self::deposit_event(Event::AddBonded(
                who.clone(),
                machine_id.clone(),
                bond_amount,
            ));

            let item = StakingLedger {
                stash: who.clone(),
                total: bond_amount,
                active: bond_amount,
                unlocking: vec![],
                claimed_rewards: (last_reward_era..current_era).collect(),
                released_rewards: 0u32.into(),
                upcoming_rewards: VecDeque::new(),
            };

            Self::update_ledger(&who, &machine_id, &item);

            Ok(().into())
        }

        #[pallet::weight(10000)]
        fn bond_extra(
            origin: OriginFor<T>,
            machine_id: MachineId,
            max_additional: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let mut ledger = Self::ledger(&who, &machine_id).ok_or(Error::<T>::LedgerNotFound)?;
            let user_balance = T::Currency::free_balance(&who);
            if let Some(extra) = user_balance.checked_sub(&ledger.total) {
                let extra = extra.min(max_additional);
                ledger.total += extra;
                ledger.active += extra;

                ensure!(ledger.active >= T::Currency::minimum_balance(), Error::<T>::InsufficientValue);

                Self::deposit_event(Event::AddBonded(who.clone(), machine_id.clone(), extra));
                Self::update_ledger(&who, &machine_id, &ledger);
            }

            Ok(().into())
        }

        #[pallet::weight(10000)]
        fn reduce_bonded_token(
            origin: OriginFor<T>,
            machine_id: MachineId,
            amount: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let mut ledger = Self::ledger(&who, &machine_id).ok_or(Error::<T>::LedgerNotFound)?;

            ensure!(ledger.unlocking.len() < crate::MAX_UNLOCKING_CHUNKS, Error::<T>::NoMoreChunks);
            let mut value = amount.min(ledger.active);

            if !value.is_zero() {
                ledger.active -= value;

                if ledger.active < <T as Config>::Currency::minimum_balance() {
                    value += ledger.active;
                    ledger.active = Zero::zero();
                }

                let era = <random_num::Module<T>>::current_era() + T::BondingDuration::get();
                ledger.unlocking.push(UnlockChunk { value, era });
                Self::update_ledger(&who, &machine_id, &ledger);
                Self::deposit_event(Event::RemoveBonded(who, machine_id, value));
            }

            Ok(().into())
        }

        #[pallet::weight(10000)]
        fn withdraw_unbonded(
            origin: OriginFor<T>,
            machine_id: MachineId,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let mut ledger = Self::ledger(&who, &machine_id).ok_or(Error::<T>::LedgerNotFound)?;

            let old_total = ledger.total;
            let current_era = <random_num::Module<T>>::current_era();
            ledger = ledger.consolidate_unlock(current_era);
            if ledger.unlocking.is_empty() && ledger.active <= T::Currency::minimum_balance() {
                // TODO: 清除ledger相关存储
                T::Currency::remove_lock(crate::PALLET_LOCK_ID, &who);
            } else {
                Self::update_ledger(&who, &machine_id, &ledger);
            }

            if ledger.total < old_total {
                let value = old_total - ledger.total;
                Self::deposit_event(Event::Withdrawn(who, machine_id, value));
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
        BondMachine(T::AccountId, MachineId),
        AddBonded(T::AccountId, MachineId, BalanceOf<T>),
        RemoveBonded(T::AccountId, MachineId, BalanceOf<T>),
        DonationReceived(T::AccountId, BalanceOf<T>, BalanceOf<T>),
        FundsAllocated(T::AccountId, BalanceOf<T>, BalanceOf<T>),
        Withdrawn(T::AccountId, MachineId, BalanceOf<T>),
    }

    #[pallet::error]
    pub enum Error<T> {
        MachineIdNotBonded,
        MachineHasBonded,
        MachineInBondingMachine,
        MachineInBookingMachine,
        MachineInBookedMachine,
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

    // Update min_stake_dbc every block end
    fn update_min_stake_dbc() {
        // TODO: 1. 获取DBC价格
        let dbc_price = dbc_price_ocw::Pallet::<T>::avg_price;

        // TODO: 2. 计算所需DBC
        // TODO: 3. 更新min_stake_dbc变量
    }

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

    fn end_era() {}

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

impl<T: Config> LCOps for Pallet<T> {
    type MachineId = MachineId;
    type AccountId = T::AccountId;
    type BlockNumber = T::BlockNumber;

    fn bonding_queue_id() -> BTreeSet<Self::MachineId> {
        <BondingMachine<T> as IterableStorageMap<MachineId, BondingPair<T::AccountId>>>::iter()
            .map(|(machine_id, _)| machine_id)
            .collect::<BTreeSet<_>>()
    }

    fn booking_queue_id() -> BTreeSet<Self::MachineId> {
        <BookingMachine<T> as IterableStorageMap<MachineId, BookingItem<T::BlockNumber>>>::iter()
            .map(|(machine_id, _)| machine_id)
            .collect::<BTreeSet<_>>()
    }

    fn book_one_machine(_who: &T::AccountId, machine_id: MachineId) -> bool {
        let bonding_queue_id = Self::bonding_queue_id();
        if !bonding_queue_id.contains(&machine_id) {
            return false;
        }

        let booking_item = BookingItem {
            machine_id: machine_id.to_vec(),
            book_time: <frame_system::Module<T>>::block_number(),
        };

        BookingMachine::<T>::insert(&machine_id, booking_item.clone());
        BondingMachine::<T>::remove(&machine_id);
        true
    }

    fn booked_queue_id() -> BTreeSet<Self::MachineId> {
        <BookedMachine<T> as IterableStorageMap<MachineId, u64>>::iter()
            .map(|(machine_id, _)| machine_id)
            .collect::<BTreeSet<_>>()
    }

    fn bonded_machine_id() -> BTreeSet<Self::MachineId> {
        <BondedMachine<T> as IterableStorageMap<MachineId, ()>>::iter()
            .map(|(machine_id, _)| machine_id)
            .collect::<BTreeSet<_>>()
    }

    fn rm_booking_id(id: MachineId) {
        BookingMachine::<T>::remove(id);
    }

    fn add_booked_id(_id: MachineId) {}

    fn confirm_machine_grade(who: T::AccountId, machine_id: MachineId, confirm: bool) {
        let mut machine_grade = OCWMachineGrades::<T>::get(&machine_id);

        machine_grade.committee_info.push(CommitteeInfo {
            account_id: who,
            block_height: <frame_system::Module<T>>::block_number(),
            confirm,
        });

        OCWMachineGrades::<T>::insert(&machine_id, machine_grade);
    }
}

impl<T: Config> OPOps for Pallet<T> {
    type AccountId = T::AccountId;
    type BookingItem = BookingItem<T::BlockNumber>;
    type BondingPair = BondingPair<T::AccountId>;
    type ConfirmedMachine = ConfirmedMachine<T::AccountId, T::BlockNumber>;
    type MachineId = MachineId;

    fn get_bonding_pair(id: Self::MachineId) -> Self::BondingPair {
        BondingMachine::<T>::get(id)
    }

    fn add_machine_grades(id: Self::MachineId, machine_grade: Self::ConfirmedMachine) {
        OCWMachineGrades::<T>::insert(id, machine_grade)
    }

    fn add_machine_price(id: Self::MachineId, price: u64) {
        OCWMachinePrice::<T>::insert(id, price)
    }

    fn rm_bonding_id(id: Self::MachineId) {
        BondingMachine::<T>::remove(id);
    }

    fn add_booking_item(id: Self::MachineId, booking_item: Self::BookingItem) {
        BookingMachine::<T>::insert(id, booking_item);
    }
}

impl<T: Config> OLProof for Pallet<T> {
    type MachineId = MachineId;

    fn staking_machine() -> BTreeSet<Self::MachineId> {
        // StakingMachine
        <StakingMachine<T> as IterableStorageMap<MachineId, Vec<bool>>>::iter()
            .map(|(machine_id, _)| machine_id)
            .collect::<BTreeSet<_>>()
    }

    // TODO: 添加这个函数实现
    fn add_verify_result(id: Self::MachineId, is_online: bool) {}
}
