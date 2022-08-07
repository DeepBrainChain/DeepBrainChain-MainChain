#![cfg_attr(not(feature = "std"), no_std)]

mod rpc;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

use codec::{Decode, Encode};
use frame_support::{
    dispatch::DispatchResult,
    ensure,
    pallet_prelude::*,
    traits::{Currency, ExistenceRequirement::KeepAlive, ReservableCurrency},
    IterableStorageMap,
};
use frame_system::{ensure_root, ensure_signed, pallet_prelude::*};
use generic_func::{ItemList, MachineId};
pub use online_profile::{EraIndex, MachineStatus};
use online_profile_machine::{DbcPrice, RTOps};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::traits::{CheckedAdd, CheckedSub, SaturatedConversion, Zero};
use sp_std::{collections::btree_set::BTreeSet, prelude::*, str, vec::Vec};

type BalanceOf<T> = <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

/// 等待30个块(15min)，用户确认是否租用成功
pub const WAITING_CONFIRMING_DELAY: u32 = 30;
/// 1天按照2880个块
pub const BLOCK_PER_DAY: u32 = 2880;

pub use pallet::*;

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct RentOrderDetail<AccountId, BlockNumber, Balance> {
    /// 租用者
    pub renter: AccountId,
    /// 租用开始时间
    pub rent_start: BlockNumber,
    /// 用户确认租成功的时间
    pub confirm_rent: BlockNumber,
    /// 租用结束时间
    pub rent_end: BlockNumber,
    /// 用户对该机器的质押
    pub stake_amount: Balance,
    /// 当前订单的状态
    pub rent_status: RentStatus,
    /// 租用的GPU数量
    pub gpu_num: u32,
}

// A: AccountId, B: BlockNumber, C: Balance
impl<A, B: Default, C: Default> RentOrderDetail<A, B, C> {
    pub fn new(renter: A, rent_start: B, rent_end: B, stake_amount: C, gpu_num: u32) -> Self {
        Self {
            renter,
            rent_start,
            confirm_rent: B::default(),
            rent_end,
            stake_amount,
            rent_status: RentStatus::WaitingVerifying,
            gpu_num
        }
    }

    pub fn confirm_rent(&mut self, confirm_rent_time: B) {
        self.confirm_rent = confirm_rent_time;
        self.stake_amount = C::default();
        self.rent_status = RentStatus::Renting;
    }
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum RentStatus {
    WaitingVerifying,
    Renting,
    RentExpired,
}

impl Default for RentStatus {
    fn default() -> Self {
        RentStatus::RentExpired
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + online_profile::Config + generic_func::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Currency: ReservableCurrency<Self::AccountId>;
        type RTOps: RTOps<
            MachineId = MachineId,
            MachineStatus = MachineStatus<Self::BlockNumber, Self::AccountId>,
            AccountId = Self::AccountId,
            Balance = BalanceOf<Self>,
        >;
        type DbcPrice: DbcPrice<Balance = BalanceOf<Self>>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(_block_number: T::BlockNumber) {
            Self::check_machine_starting_status();
            Self::check_if_rent_finished();
        }
    }

    // 存储用户当前租用的机器列表
    #[pallet::storage]
    #[pallet::getter(fn user_rented)]
    pub(super) type UserRented<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, Vec<MachineId>, ValueQuery>;

    // 用户当前租用的某个机器的详情
    #[pallet::storage]
    #[pallet::getter(fn rent_order)]
    pub(super) type RentOrder<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        MachineId,
        // TODO: 记录每个租用记录
        // Vec<RentOrderDetail<T::AccountId, T::BlockNumber, BalanceOf<T>>>,
        RentOrderDetail<T::AccountId, T::BlockNumber, BalanceOf<T>>,
        ValueQuery,
    >;

    // 等待用户确认租用成功的机器
    #[pallet::storage]
    #[pallet::getter(fn pending_confirming)]
    pub(super) type PendingConfirming<T: Config> = StorageMap<_, Blake2_128Concat, MachineId, T::AccountId, ValueQuery>;

    // 记录每个区块将要结束租用的机器
    #[pallet::storage]
    #[pallet::getter(fn pending_rent_ending)]
    pub(super) type PendingRentEnding<T: Config> =
        StorageMap<_, Blake2_128Concat, T::BlockNumber, Vec<MachineId>, ValueQuery>;

    // 存储每个用户在该模块中的总质押量
    #[pallet::storage]
    #[pallet::getter(fn user_total_stake)]
    pub(super) type UserTotalStake<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

    // 租金支付目标地址
    #[pallet::storage]
    #[pallet::getter(fn rent_fee_pot)]
    pub(super) type RentFeePot<T: Config> = StorageValue<_, T::AccountId>;

    #[pallet::type_value]
    pub(super) fn MaximumRentalDurationDefault<T: Config>() -> EraIndex {
        60
    }

    // 最大租用/续租用时间
    #[pallet::storage]
    #[pallet::getter(fn maximum_rental_duration)]
    pub(super) type MaximumRentalDuration<T: Config> =
        StorageValue<_, EraIndex, ValueQuery, MaximumRentalDurationDefault<T>>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // 设置机器租金支付目标地址
        #[pallet::weight(0)]
        pub fn set_rent_fee_pot(origin: OriginFor<T>, pot_addr: T::AccountId) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            RentFeePot::<T>::put(pot_addr);
            Ok(().into())
        }

        /// 用户租用机器（按分钟租用）
        #[pallet::weight(10000)]
        pub fn rent_machine_by_minutes(
            origin: OriginFor<T>,
            machine_id: MachineId,
            rent_gpu_num: u32,
            minutes: u32,
        ) -> DispatchResultWithPostInfo {
            let renter = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();
            let machine_info = <online_profile::Module<T>>::machines_info(&machine_id);
            let machine_rented_gpu = <online_profile::Module<T>>::machine_rented_gpu(&machine_id);

            ensure!(minutes % 30 == 0, Error::<T>::OnlyHalfHourAllowed);

            // 检查machine_id状态是否可以租用
            ensure!(machine_info.machine_status == MachineStatus::Online, Error::<T>::MachineNotRentable);
            // 检查还有空闲的GPU
            let gpu_num = machine_info.machine_info_detail.committee_upload_info.gpu_num;
            ensure!(rent_gpu_num + machine_rented_gpu <= gpu_num, Error::<T>::GPUNotEnough);

            // 最大租用时间限制MaximumRentalDuration
            let duration = minutes.min(Self::maximum_rental_duration() * 24 * 60);

            // NOTE: 用户提交订单，需要扣除10个DBC
            <generic_func::Module<T>>::pay_fixed_tx_fee(renter.clone()).map_err(|_| Error::<T>::PayTxFeeFailed)?;

            // 获得machine_price(每天的价格)
            // FIXME: 根据租用GPU数量计算价格
            let machine_price =
                T::RTOps::get_machine_price(machine_info.machine_info_detail.committee_upload_info.calc_point)
                    .ok_or(Error::<T>::GetMachinePriceFailed)?;

            // 根据租用时长计算rent_fee
            let rent_fee_value = machine_price
                .checked_mul(duration as u64)
                .ok_or(Error::<T>::Overflow)?
                .checked_div(24 * 60)
                .ok_or(Error::<T>::Overflow)?;
            let rent_fee =
                <T as pallet::Config>::DbcPrice::get_dbc_amount_by_value(rent_fee_value).ok_or(Error::<T>::Overflow)?;

            // 获取用户租用的结束时间(块高)
            let rent_end = T::BlockNumber::from(minutes.checked_mul(2).ok_or(Error::<T>::Overflow)?)
                .checked_add(&now)
                .ok_or(Error::<T>::Overflow)?;

            // 质押用户的资金，并修改机器状态
            Self::change_renter_total_stake(&renter, rent_fee, true).map_err(|_| Error::<T>::InsufficientValue)?;

            RentOrder::<T>::insert(&machine_id, RentOrderDetail::new(renter.clone(), now, rent_end, rent_fee, gpu_num));

            let mut user_rented = Self::user_rented(&renter);
            ItemList::add_item(&mut user_rented, machine_id.clone());
            UserRented::<T>::insert(&renter, user_rented);

            let mut pending_rent_ending = Self::pending_rent_ending(rent_end);
            ItemList::add_item(&mut pending_rent_ending, machine_id.clone());
            PendingRentEnding::<T>::insert(rent_end, pending_rent_ending);

            // 改变online_profile状态，影响机器佣金
            // TODO: 改变租用的GPU数量
            T::RTOps::change_machine_status(&machine_id, MachineStatus::Creating, Some(renter.clone()), None, rent_gpu_num);

            PendingConfirming::<T>::insert(&machine_id, renter.clone());

            Self::deposit_event(Event::RentBlockNum(renter, machine_id, rent_fee, duration.into()));
            Ok(().into())
        }

        /// 用户租用机器(按天租用)
        #[pallet::weight(10000)]
        pub fn rent_machine(
            origin: OriginFor<T>,
            machine_id: MachineId,
            rent_gpu_num: u32,
            duration: EraIndex,
        ) -> DispatchResultWithPostInfo {
            let renter = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();
            let machine_info = <online_profile::Module<T>>::machines_info(&machine_id);
            let machine_rented_gpu = <online_profile::Module<T>>::machine_rented_gpu(&machine_id);

            // 检查machine_id状态是否可以租用
            ensure!(machine_info.machine_status == MachineStatus::Online, Error::<T>::MachineNotRentable);
            // 检查还有空闲的GPU
            let gpu_num = machine_info.machine_info_detail.committee_upload_info.gpu_num;
            ensure!(rent_gpu_num + machine_rented_gpu <= gpu_num, Error::<T>::GPUNotEnough);

            // 最大租用时间限制MaximumRentalDuration
            let duration = duration.min(Self::maximum_rental_duration());

            // 用户提交订单，需要扣除10个DBC
            <generic_func::Module<T>>::pay_fixed_tx_fee(renter.clone()).map_err(|_| Error::<T>::PayTxFeeFailed)?;

            // 获得machine_price
            // FIXME: 根据租用GPU数量计算价格
            let machine_price =
                T::RTOps::get_machine_price(machine_info.machine_info_detail.committee_upload_info.calc_point)
                    .ok_or(Error::<T>::GetMachinePriceFailed)?;

            let rent_fee_value = machine_price.checked_mul(duration as u64).ok_or(Error::<T>::Overflow)?;
            let rent_fee =
                <T as pallet::Config>::DbcPrice::get_dbc_amount_by_value(rent_fee_value).ok_or(Error::<T>::Overflow)?;

            // 获取用户租用的结束时间
            let rent_end = BLOCK_PER_DAY
                .checked_mul(duration)
                .ok_or(Error::<T>::Overflow)?
                .saturated_into::<T::BlockNumber>()
                .checked_add(&now)
                .ok_or(Error::<T>::Overflow)?;

            // 质押用户的资金，并修改机器状态
            Self::change_renter_total_stake(&renter, rent_fee, true).map_err(|_| Error::<T>::InsufficientValue)?;

            RentOrder::<T>::insert(&machine_id, RentOrderDetail::new(renter.clone(), now, rent_end, rent_fee, gpu_num));

            let mut user_rented = Self::user_rented(&renter);
            ItemList::add_item(&mut user_rented, machine_id.clone());
            UserRented::<T>::insert(&renter, user_rented);

            let mut pending_rent_ending = Self::pending_rent_ending(rent_end);
            ItemList::add_item(&mut pending_rent_ending, machine_id.clone());
            PendingRentEnding::<T>::insert(rent_end, pending_rent_ending);

            // 改变online_profile状态，影响机器佣金
            T::RTOps::change_machine_status(&machine_id, MachineStatus::Creating, Some(renter.clone()), None, gpu_num);

            PendingConfirming::<T>::insert(&machine_id, renter.clone());
            Self::deposit_event(Event::RentMachine(renter, machine_id, rent_fee, duration));
            Ok(().into())
        }

        /// 用户在租用15min(30个块)内确认机器租用成功
        #[pallet::weight(10000)]
        pub fn confirm_rent(origin: OriginFor<T>, machine_id: MachineId) -> DispatchResultWithPostInfo {
            let renter = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            let mut order_info = Self::rent_order(&machine_id);
            ensure!(order_info.renter == renter, Error::<T>::NoOrderExist);
            ensure!(order_info.rent_status == RentStatus::WaitingVerifying, Error::<T>::NoOrderExist);

            // 不能超过15分钟
            let machine_start_duration = now.checked_sub(&order_info.rent_start).ok_or(Error::<T>::Overflow)?;
            ensure!(machine_start_duration <= WAITING_CONFIRMING_DELAY.into(), Error::<T>::ExpiredConfirm);

            let machine_info = <online_profile::Module<T>>::machines_info(&machine_id);
            ensure!(machine_info.machine_status == MachineStatus::Creating, Error::<T>::StatusNotAllowed);

            // 质押转到特定账户
            Self::change_renter_total_stake(&renter, order_info.stake_amount, false)
                .map_err(|_| Error::<T>::UnlockToPayFeeFailed)?;
            Self::pay_rent_fee(&renter, machine_id.clone(), machine_info.machine_stash, order_info.stake_amount)?;

            // 在stake_amount设置0前记录，用作事件
            let rent_fee = order_info.stake_amount;
            let rent_duration = order_info.rent_end - order_info.rent_start;

            order_info.confirm_rent(now);
            RentOrder::<T>::insert(&machine_id, order_info);

            // 改变online_profile状态
            // FIXME: 修改gpu_num
            T::RTOps::change_machine_status(&machine_id, MachineStatus::Rented, Some(renter.clone()), None, 0);
            PendingConfirming::<T>::remove(&machine_id);

            Self::deposit_event(Event::ConfirmReletBlockNum(renter, machine_id, rent_fee, rent_duration));
            Ok(().into())
        }

        /// 用户续租(按分钟续租)
        #[pallet::weight(10000)]
        pub fn relet_machine_by_minutes(
            origin: OriginFor<T>,
            machine_id: MachineId,
            minutes: u32, // 分钟
        ) -> DispatchResultWithPostInfo {
            let renter = ensure_signed(origin)?;
            let mut order_info = Self::rent_order(&machine_id);
            let old_rent_end = order_info.rent_end;

            ensure!(minutes % 30 == 0, Error::<T>::OnlyHalfHourAllowed);
            ensure!(order_info.renter == renter, Error::<T>::NoOrderExist);
            ensure!(order_info.rent_status == RentStatus::Renting, Error::<T>::NoOrderExist);

            let machine_info = <online_profile::Module<T>>::machines_info(&machine_id);
            let calc_point = machine_info.machine_info_detail.committee_upload_info.calc_point;

            // 确保租用时间不超过设定的限制，计算最多续费租用到
            let now = <frame_system::Module<T>>::block_number();
            // 最大结束块高为 今天租用开始的时间 + 60天
            // 60 days * 24 hour/day * 60 min/hour * 2 block/min
            let max_rent_end = now.checked_add(&(60u32 * 24 * 60 * 2).into()).ok_or(Error::<T>::Overflow)?;
            let wanted_rent_end = old_rent_end + (minutes * 2).into();

            // 计算实际续租了多久 (块高)
            let add_duration: T::BlockNumber =
                if max_rent_end >= wanted_rent_end { (minutes * 2).into() } else { (60u32 * 24 * 60 * 2).into() };

            if add_duration == 0u32.into() {
                return Ok(().into());
            }

            // 计算rent_fee
            let machine_price = T::RTOps::get_machine_price(calc_point).ok_or(Error::<T>::GetMachinePriceFailed)?;
            let rent_fee_value = machine_price
                .checked_mul(add_duration.saturated_into::<u64>())
                .ok_or(Error::<T>::Overflow)?
                .checked_div(2880)
                .ok_or(Error::<T>::Overflow)?;
            let rent_fee =
                <T as pallet::Config>::DbcPrice::get_dbc_amount_by_value(rent_fee_value).ok_or(Error::<T>::Overflow)?;

            // 检查用户是否有足够的资金，来租用机器
            let user_balance = <T as pallet::Config>::Currency::free_balance(&renter);
            ensure!(rent_fee < user_balance, Error::<T>::InsufficientValue);

            Self::pay_rent_fee(&renter, machine_id.clone(), machine_info.machine_stash, rent_fee)?;

            // 获取用户租用的结束时间
            order_info.rent_end = order_info.rent_end.checked_add(&add_duration).ok_or(Error::<T>::Overflow)?;

            let mut old_pending_rent_ending = Self::pending_rent_ending(old_rent_end);
            ItemList::rm_item(&mut old_pending_rent_ending, &machine_id);
            let mut pending_rent_ending = Self::pending_rent_ending(order_info.rent_end);
            ItemList::add_item(&mut pending_rent_ending, machine_id.clone());

            PendingRentEnding::<T>::insert(old_rent_end, old_pending_rent_ending);
            PendingRentEnding::<T>::insert(order_info.rent_end, pending_rent_ending);
            RentOrder::<T>::insert(&machine_id, order_info);

            Self::deposit_event(Event::ReletBlockNum(renter, machine_id, rent_fee, add_duration));
            Ok(().into())
        }

        /// 用户续租(按天续租)
        /// FIXME: 如果用户租用同一个机器多次
        /// 修改成通过order_id来续租
        #[pallet::weight(10000)]
        pub fn relet_machine(
            origin: OriginFor<T>,
            machine_id: MachineId,
            add_duration: EraIndex,
        ) -> DispatchResultWithPostInfo {
            let renter = ensure_signed(origin)?;

            let mut order_info = Self::rent_order(&machine_id);
            let old_rent_end = order_info.rent_end;

            ensure!(order_info.renter == renter, Error::<T>::NoOrderExist);
            ensure!(order_info.rent_status == RentStatus::Renting, Error::<T>::NoOrderExist);

            let machine_info = <online_profile::Module<T>>::machines_info(&machine_id);
            let calc_point = machine_info.machine_info_detail.committee_upload_info.calc_point;

            // 确保租用时间不超过设定的限制
            let now = <frame_system::Module<T>>::block_number();
            // 最大结束块高为 今天租用开始的时间 + 60天
            let max_rent_end = order_info.rent_start
                + (now - order_info.rent_start) / BLOCK_PER_DAY.into() * BLOCK_PER_DAY.into()
                + (Self::maximum_rental_duration() * BLOCK_PER_DAY).into();
            let wanted_rent_end = old_rent_end + (add_duration * BLOCK_PER_DAY).into();

            let add_duration = if wanted_rent_end >= max_rent_end {
                (max_rent_end - old_rent_end).saturated_into::<u64>() / BLOCK_PER_DAY as u64
            } else {
                add_duration as u64
            };

            if add_duration == 0 {
                return Ok(().into());
            }

            let machine_price = T::RTOps::get_machine_price(calc_point).ok_or(Error::<T>::GetMachinePriceFailed)?;
            let rent_fee_value = machine_price.checked_mul(add_duration as u64).ok_or(Error::<T>::Overflow)?;
            let rent_fee =
                <T as pallet::Config>::DbcPrice::get_dbc_amount_by_value(rent_fee_value).ok_or(Error::<T>::Overflow)?;

            // 检查用户是否有足够的资金，来租用机器
            let user_balance = <T as pallet::Config>::Currency::free_balance(&renter);
            ensure!(rent_fee < user_balance, Error::<T>::InsufficientValue);

            Self::pay_rent_fee(&renter, machine_id.clone(), machine_info.machine_stash, rent_fee)?;

            // 获取用户租用的结束时间
            // rent_end = block_per_day * rent_duration + rent_end
            order_info.rent_end = (BLOCK_PER_DAY as u64)
                .checked_mul(add_duration)
                .ok_or(Error::<T>::Overflow)?
                .saturated_into::<T::BlockNumber>()
                .checked_add(&order_info.rent_end)
                .ok_or(Error::<T>::Overflow)?;

            let mut old_pending_rent_ending = Self::pending_rent_ending(old_rent_end);
            ItemList::rm_item(&mut old_pending_rent_ending, &machine_id);
            let mut pending_rent_ending = Self::pending_rent_ending(order_info.rent_end);
            ItemList::add_item(&mut pending_rent_ending, machine_id.clone());

            PendingRentEnding::<T>::insert(old_rent_end, old_pending_rent_ending);
            PendingRentEnding::<T>::insert(order_info.rent_end, pending_rent_ending);
            RentOrder::<T>::insert(&machine_id, order_info);

            Self::deposit_event(Event::ReletMachine(renter, machine_id, rent_fee, add_duration as u32));
            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        PayTxFee(T::AccountId, BalanceOf<T>),
        ConfirmRent(T::AccountId, MachineId, BalanceOf<T>, EraIndex),
        ReletMachine(T::AccountId, MachineId, BalanceOf<T>, EraIndex),
        RentMachine(T::AccountId, MachineId, BalanceOf<T>, EraIndex),
        RentBlockNum(T::AccountId, MachineId, BalanceOf<T>, T::BlockNumber),
        ReletBlockNum(T::AccountId, MachineId, BalanceOf<T>, T::BlockNumber),
        ConfirmReletBlockNum(T::AccountId, MachineId, BalanceOf<T>, T::BlockNumber),
    }

    #[pallet::error]
    pub enum Error<T> {
        AccountAlreadyExist,
        MachineNotRentable,
        Overflow,
        InsufficientValue,
        ExpiredConfirm,
        NoOrderExist,
        StatusNotAllowed,
        UnlockToPayFeeFailed,
        UndefinedRentPot,
        PayTxFeeFailed,
        GetMachinePriceFailed,
        OnlyHalfHourAllowed,
        GPUNotEnough,
    }
}

impl<T: Config> Pallet<T> {
    // NOTE: 银河竞赛开启前，租金付给stash账户；开启后租金转到销毁账户
    fn pay_rent_fee(
        renter: &T::AccountId,
        machine_id: MachineId,
        machine_stash: T::AccountId,
        fee_amount: BalanceOf<T>,
    ) -> DispatchResult {
        let rent_fee_pot = Self::rent_fee_pot().ok_or(Error::<T>::UndefinedRentPot)?;
        let galaxy_is_on = <online_profile::Module<T>>::galaxy_is_on();
        let rent_fee_to = if galaxy_is_on { rent_fee_pot } else { machine_stash };

        <T as pallet::Config>::Currency::transfer(renter, &rent_fee_to, fee_amount, KeepAlive)?;
        T::RTOps::change_machine_rent_fee(fee_amount, machine_id, galaxy_is_on);
        Ok(())
    }

    // 定时检查机器是否30分钟没有上线
    fn check_machine_starting_status() {
        let now = <frame_system::Module<T>>::block_number();

        let pending_confirming = Self::get_pending_confirming_order();
        for (machine_id, renter) in pending_confirming {
            let rent_order = Self::rent_order(&machine_id);
            let duration = now.checked_sub(&rent_order.rent_start).unwrap_or_default();

            if duration > WAITING_CONFIRMING_DELAY.into() {
                // 超过了30个块，也就是15分钟
                Self::clean_order(&renter, &machine_id);
                // FIXME: 修改gpu_num
                T::RTOps::change_machine_status(&machine_id, MachineStatus::Online, None, None, 0);
                continue;
            }
        }
    }

    fn clean_order(who: &T::AccountId, machine_id: &MachineId) {
        let mut rent_machine_list = Self::user_rented(who);
        ItemList::rm_item(&mut rent_machine_list, machine_id);

        let rent_order = Self::rent_order(machine_id);

        // return back staked money!
        if !rent_order.stake_amount.is_zero() {
            let _ = Self::change_renter_total_stake(who, rent_order.stake_amount, false);
        }

        let mut pending_rent_ending = Self::pending_rent_ending(rent_order.rent_end);
        ItemList::rm_item(&mut pending_rent_ending, machine_id);

        PendingRentEnding::<T>::insert(rent_order.rent_end, pending_rent_ending);
        RentOrder::<T>::remove(machine_id);
        UserRented::<T>::insert(who, rent_machine_list);
        PendingConfirming::<T>::remove(machine_id);
    }

    fn get_pending_confirming_order() -> BTreeSet<(MachineId, T::AccountId)> {
        <PendingConfirming<T> as IterableStorageMap<MachineId, T::AccountId>>::iter()
            .map(|(machine, acct)| (machine, acct))
            .collect::<BTreeSet<_>>()
    }

    fn change_renter_total_stake(who: &T::AccountId, amount: BalanceOf<T>, is_add: bool) -> Result<(), ()> {
        let current_stake = Self::user_total_stake(who);

        let new_stake = if is_add {
            ensure!(<T as pallet::Config>::Currency::can_reserve(who, amount), ());
            <T as pallet::Config>::Currency::reserve(who, amount).map_err(|_| ())?;
            current_stake.checked_add(&amount).ok_or(())?
        } else {
            ensure!(current_stake >= amount, ());
            let _ = <T as pallet::Config>::Currency::unreserve(who, amount);
            current_stake.checked_sub(&amount).ok_or(())?
        };
        UserTotalStake::<T>::insert(who, new_stake);
        Ok(())
    }

    // 这里修rentMachine模块通知onlineProfile机器已经租用完成，
    // onlineProfile判断机器是否需要变成online状态，或者记录下之前是租用状态，以便机器再次上线时进行正确的惩罚
    fn check_if_rent_finished() {
        let now = <frame_system::Module<T>>::block_number();
        let pending_ending = Self::pending_rent_ending(now);

        for machine_id in pending_ending {
            let rent_order = Self::rent_order(&machine_id);
            let rent_duration: u64 = (now - rent_order.rent_start).saturated_into::<u64>() / 2880;

            // FIXME: 修改gpu_num
            T::RTOps::change_machine_status(
                &machine_id,
                MachineStatus::Online,
                Some(rent_order.renter.clone()),
                Some(rent_duration),
                0,
            );
            Self::clean_order(&rent_order.renter, &machine_id);
        }
    }
}
