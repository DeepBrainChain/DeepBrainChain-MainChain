#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(unused_crate_dependencies)]

// pub mod migrations;
mod rpc;

#[cfg(test)]
mod mock;
#[allow(non_upper_case_globals)]
#[cfg(test)]
mod tests;

pub use dbc_support::machine_type::MachineStatus;
use dbc_support::{
    rental_type::{MachineGPUOrder, RentOrderDetail, RentStatus},
    traits::{DbcPrice, RTOps},
    EraIndex, ItemList, MachineId, RentOrderId, ONE_DAY,
};
use frame_support::{
    dispatch::DispatchResult,
    ensure,
    pallet_prelude::*,
    traits::{Currency, ExistenceRequirement::KeepAlive, ReservableCurrency},
};
use frame_system::{ensure_root, ensure_signed, pallet_prelude::*};
use sp_runtime::traits::{CheckedAdd, CheckedSub, SaturatedConversion, Saturating, Zero};
use sp_std::{prelude::*, str, vec::Vec};

type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

/// 等待30个块(15min)，用户确认是否租用成功
pub const WAITING_CONFIRMING_DELAY: u32 = 30;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + online_profile::Config + generic_func::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Currency: ReservableCurrency<Self::AccountId>;
        type RTOps: RTOps<
            MachineId = MachineId,
            MachineStatus = MachineStatus<Self::BlockNumber, Self::AccountId>,
            AccountId = Self::AccountId,
            Balance = BalanceOf<Self>,
            BlockNumber = Self::BlockNumber,
        >;
        type DbcPrice: DbcPrice<Balance = BalanceOf<Self>>;
    }

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(_block_number: T::BlockNumber) {
            let _ = Self::check_machine_starting_status();
            let _ = Self::check_if_rent_finished();
        }

        // fn on_runtime_upgrade() -> Weight {
        //     frame_support::debug::RuntimeLogger::init();
        //     frame_support::debug::info!("🔍️ OnlineProfile Storage Migration start");
        //     let weight1 = online_profile::migrations::apply::<T>();
        //     frame_support::debug::info!("🚀 OnlineProfile Storage Migration end");

        //     frame_support::debug::RuntimeLogger::init();
        //     frame_support::debug::info!("🔍️ RentMachine Storage Migration start");
        //     let weight2 = migrations::apply::<T>();
        //     frame_support::debug::info!("🚀 RentMachine Storage Migration end");
        //     weight1 + weight2
        // }
    }

    // 存储用户当前租用的机器列表
    #[pallet::storage]
    #[pallet::getter(fn user_order)]
    pub(super) type UserOrder<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, Vec<RentOrderId>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn machine_rent_order)]
    pub(super) type MachineRentOrder<T: Config> =
        StorageMap<_, Blake2_128Concat, MachineId, MachineGPUOrder, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn next_rent_id)]
    pub(super) type NextRentId<T: Config> = StorageValue<_, RentOrderId, ValueQuery>;

    // 用户当前租用的某个机器的详情
    // 记录每个租用记录
    #[pallet::storage]
    #[pallet::getter(fn rent_info)]
    pub type RentInfo<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        RentOrderId,
        RentOrderDetail<T::AccountId, T::BlockNumber, BalanceOf<T>>,
    >;

    // 等待用户确认租用成功的机器
    #[pallet::storage]
    #[pallet::getter(fn confirming_order)]
    pub type ConfirmingOrder<T: Config> =
        StorageMap<_, Blake2_128Concat, T::BlockNumber, Vec<RentOrderId>, ValueQuery>;

    // 记录每个区块将要结束租用的机器
    #[pallet::storage]
    #[pallet::getter(fn rent_ending)]
    pub(super) type RentEnding<T: Config> =
        StorageMap<_, Blake2_128Concat, T::BlockNumber, Vec<RentOrderId>, ValueQuery>;

    // 存储每个用户在该模块中的总质押量
    #[pallet::storage]
    #[pallet::getter(fn user_total_stake)]
    pub(super) type UserTotalStake<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

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

    // The current storage version.
    #[pallet::storage]
    #[pallet::getter(fn storage_version)]
    pub(super) type StorageVersion<T: Config> = StorageValue<_, u16, ValueQuery>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // 设置机器租金支付目标地址
        #[pallet::call_index(0)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn set_rent_fee_pot(
            origin: OriginFor<T>,
            pot_addr: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            RentFeePot::<T>::put(pot_addr);
            Ok(().into())
        }

        /// 用户租用机器(按天租用)
        #[pallet::call_index(1)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn rent_machine(
            origin: OriginFor<T>,
            machine_id: MachineId,
            rent_gpu_num: u32,
            duration: T::BlockNumber,
        ) -> DispatchResultWithPostInfo {
            let renter = ensure_signed(origin)?;
            Self::rent_machine_by_block(renter, machine_id, rent_gpu_num, duration)
        }

        /// 用户在租用15min(30个块)内确认机器租用成功
        #[pallet::call_index(2)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn confirm_rent(
            origin: OriginFor<T>,
            rent_id: RentOrderId,
        ) -> DispatchResultWithPostInfo {
            let renter = ensure_signed(origin)?;
            let now = <frame_system::Pallet<T>>::block_number();

            let mut rent_info = Self::rent_info(&rent_id).ok_or(Error::<T>::Unknown)?;
            let machine_id = rent_info.machine_id.clone();
            let gpu_num = rent_info.gpu_num.clone();
            ensure!(rent_info.renter == renter, Error::<T>::NoOrderExist);
            ensure!(
                rent_info.rent_status == RentStatus::WaitingVerifying,
                Error::<T>::NoOrderExist
            );

            // 不能超过15分钟
            let machine_start_duration =
                now.checked_sub(&rent_info.rent_start).ok_or(Error::<T>::Overflow)?;
            ensure!(
                machine_start_duration <= WAITING_CONFIRMING_DELAY.into(),
                Error::<T>::ExpiredConfirm
            );

            let machine_info = <online_profile::Pallet<T>>::machines_info(&machine_id)
                .ok_or(Error::<T>::Unknown)?;
            ensure!(
                machine_info.machine_status == MachineStatus::Rented,
                Error::<T>::StatusNotAllowed
            );

            // 质押转到特定账户
            Self::change_renter_total_stake(&renter, rent_info.stake_amount, false)
                .map_err(|_| Error::<T>::UnlockToPayFeeFailed)?;
            Self::pay_rent_fee(
                &renter,
                machine_id.clone(),
                machine_info.machine_stash,
                rent_info.stake_amount,
            )?;

            // 在stake_amount设置0前记录，用作事件
            let rent_fee = rent_info.stake_amount;
            let rent_duration = rent_info.rent_end.saturating_sub(rent_info.rent_start);

            rent_info.confirm_rent(now);
            rent_info.stake_amount = Default::default();

            // 改变online_profile状态
            T::RTOps::change_machine_status_on_confirmed(&machine_id, renter.clone())
                .map_err(|_| Error::<T>::Unknown)?;

            ConfirmingOrder::<T>::mutate(
                rent_info.rent_start + WAITING_CONFIRMING_DELAY.into(),
                |pending_confirming| {
                    ItemList::rm_item(pending_confirming, &rent_id);
                },
            );
            RentInfo::<T>::insert(&rent_id, rent_info);

            Self::deposit_event(Event::ConfirmRent(
                rent_id,
                renter,
                machine_id,
                gpu_num,
                rent_duration,
                rent_fee,
            ));
            Ok(().into())
        }

        /// 用户续租(按天续租), 通过order_id来续租
        #[pallet::call_index(3)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn relet_machine(
            origin: OriginFor<T>,
            rent_id: RentOrderId,
            relet_duration: T::BlockNumber,
        ) -> DispatchResultWithPostInfo {
            let renter = ensure_signed(origin)?;
            Self::relet_machine_by_block(renter, rent_id, relet_duration)
        }
    }

    #[pallet::event]
    // #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        PayTxFee(T::AccountId, BalanceOf<T>),
        // rent_id, renter, MachineId, gpu_num, duration, balance
        ConfirmRent(RentOrderId, T::AccountId, MachineId, u32, T::BlockNumber, BalanceOf<T>),
        // rent_id, renter, MachineId, gpu_num, duration, balance
        Rent(RentOrderId, T::AccountId, MachineId, u32, T::BlockNumber, BalanceOf<T>),
        // rent_id, renter, MachineId, gpu_num, duration, balance
        Relet(RentOrderId, T::AccountId, MachineId, u32, T::BlockNumber, BalanceOf<T>),
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
        NotMachineRenter,
        Unknown,
        ReletTooShort,
    }
}

impl<T: Config> Pallet<T> {
    fn rent_machine_by_block(
        renter: T::AccountId,
        machine_id: MachineId,
        rent_gpu_num: u32,
        duration: T::BlockNumber,
    ) -> DispatchResultWithPostInfo {
        let now = <frame_system::Pallet<T>>::block_number();
        let machine_info =
            <online_profile::Pallet<T>>::machines_info(&machine_id).ok_or(Error::<T>::Unknown)?;
        let machine_rented_gpu = <online_profile::Pallet<T>>::machine_rented_gpu(&machine_id);
        let gpu_num = machine_info.gpu_num();

        if gpu_num == 0 || duration == Zero::zero() {
            return Ok(().into());
        }

        // 检查还有空闲的GPU
        ensure!(rent_gpu_num + machine_rented_gpu <= gpu_num, Error::<T>::GPUNotEnough);

        // 租用必须是30min的整数倍
        ensure!(duration % 60u32.into() == Zero::zero(), Error::<T>::OnlyHalfHourAllowed);

        // 检查machine_id状态是否可以租用
        ensure!(
            machine_info.machine_status == MachineStatus::Online ||
                machine_info.machine_status == MachineStatus::Rented,
            Error::<T>::MachineNotRentable
        );

        // 最大租用时间限制MaximumRentalDuration
        let duration =
            duration.min((Self::maximum_rental_duration().saturating_mul(ONE_DAY)).into());

        // NOTE: 用户提交订单，需要扣除10个DBC
        <generic_func::Pallet<T>>::pay_fixed_tx_fee(renter.clone())
            .map_err(|_| Error::<T>::PayTxFeeFailed)?;

        // 获得machine_price(每天的价格)
        // 根据租用GPU数量计算价格
        let machine_price =
            T::RTOps::get_machine_price(machine_info.calc_point(), rent_gpu_num, gpu_num)
                .ok_or(Error::<T>::GetMachinePriceFailed)?;

        // 根据租用时长计算rent_fee
        let rent_fee_value = machine_price
            .checked_mul(duration.saturated_into::<u64>())
            .ok_or(Error::<T>::Overflow)?
            .checked_div(ONE_DAY as u64)
            .ok_or(Error::<T>::Overflow)?;
        let rent_fee = <T as Config>::DbcPrice::get_dbc_amount_by_value(rent_fee_value)
            .ok_or(Error::<T>::Overflow)?;

        // 获取用户租用的结束时间(块高)
        let rent_end = duration.checked_add(&now).ok_or(Error::<T>::Overflow)?;

        // 质押用户的资金，并修改机器状态
        Self::change_renter_total_stake(&renter, rent_fee, true)
            .map_err(|_| Error::<T>::InsufficientValue)?;

        let rent_id = Self::get_new_rent_id();

        let mut machine_rent_order = Self::machine_rent_order(&machine_id);
        let rentable_gpu_index = machine_rent_order.gen_rentable_gpu(rent_gpu_num, gpu_num);
        ItemList::add_item(&mut machine_rent_order.rent_order, rent_id);

        // 改变online_profile状态，影响机器佣金
        T::RTOps::change_machine_status_on_rent_start(&machine_id, rent_gpu_num)
            .map_err(|_| Error::<T>::Unknown)?;

        RentInfo::<T>::insert(
            &rent_id,
            RentOrderDetail::new(
                machine_id.clone(),
                renter.clone(),
                now,
                rent_end,
                rent_fee,
                rent_gpu_num,
                rentable_gpu_index,
            ),
        );

        UserOrder::<T>::mutate(&renter, |user_order| {
            ItemList::add_item(user_order, rent_id);
        });

        RentEnding::<T>::mutate(rent_end, |rent_ending| {
            ItemList::add_item(rent_ending, rent_id);
        });

        ConfirmingOrder::<T>::mutate(now + WAITING_CONFIRMING_DELAY.into(), |pending_confirming| {
            ItemList::add_item(pending_confirming, rent_id);
        });

        MachineRentOrder::<T>::insert(&machine_id, machine_rent_order);

        Self::deposit_event(Event::Rent(
            rent_id,
            renter,
            machine_id,
            rent_gpu_num,
            duration.into(),
            rent_fee,
        ));
        Ok(().into())
    }

    fn relet_machine_by_block(
        renter: T::AccountId,
        rent_id: RentOrderId,
        duration: T::BlockNumber,
    ) -> DispatchResultWithPostInfo {
        let mut rent_info = Self::rent_info(&rent_id).ok_or(Error::<T>::Unknown)?;
        let old_rent_end = rent_info.rent_end;
        let machine_id = rent_info.machine_id.clone();
        let gpu_num = rent_info.gpu_num;

        // 续租允许10分钟及以上
        ensure!(duration >= 20u32.into(), Error::<T>::ReletTooShort);
        ensure!(rent_info.renter == renter, Error::<T>::NotMachineRenter);
        ensure!(rent_info.rent_status == RentStatus::Renting, Error::<T>::NoOrderExist);

        let machine_info =
            <online_profile::Pallet<T>>::machines_info(&machine_id).ok_or(Error::<T>::Unknown)?;
        let calc_point = machine_info.calc_point();

        // 确保租用时间不超过设定的限制，计算最多续费租用到
        let now = <frame_system::Pallet<T>>::block_number();
        // 最大结束块高为 今天租用开始的时间 + 60天
        // 60 days * 24 hour/day * 60 min/hour * 2 block/min
        let max_rent_end = now.checked_add(&(ONE_DAY * 60).into()).ok_or(Error::<T>::Overflow)?;
        let wanted_rent_end = old_rent_end + duration;

        // 计算实际可续租时间 (块高)
        let add_duration: T::BlockNumber = if max_rent_end >= wanted_rent_end {
            duration
        } else {
            max_rent_end.saturating_sub(old_rent_end)
        };

        if add_duration == 0u32.into() {
            return Ok(().into());
        }

        // 计算rent_fee
        let machine_price =
            T::RTOps::get_machine_price(calc_point, gpu_num, machine_info.gpu_num())
                .ok_or(Error::<T>::GetMachinePriceFailed)?;
        let rent_fee_value = machine_price
            .checked_mul(add_duration.saturated_into::<u64>())
            .ok_or(Error::<T>::Overflow)?
            .checked_div(2880)
            .ok_or(Error::<T>::Overflow)?;
        let rent_fee = <T as Config>::DbcPrice::get_dbc_amount_by_value(rent_fee_value)
            .ok_or(Error::<T>::Overflow)?;

        // 检查用户是否有足够的资金，来租用机器
        let user_balance = <T as Config>::Currency::free_balance(&renter);
        ensure!(rent_fee < user_balance, Error::<T>::InsufficientValue);

        Self::pay_rent_fee(&renter, machine_id.clone(), machine_info.machine_stash, rent_fee)?;

        // 获取用户租用的结束时间
        rent_info.rent_end =
            rent_info.rent_end.checked_add(&add_duration).ok_or(Error::<T>::Overflow)?;

        RentEnding::<T>::mutate(old_rent_end, |old_rent_ending| {
            ItemList::rm_item(old_rent_ending, &rent_id);
        });
        RentEnding::<T>::mutate(rent_info.rent_end, |rent_ending| {
            ItemList::add_item(rent_ending, rent_id);
        });

        RentInfo::<T>::insert(&rent_id, rent_info);

        Self::deposit_event(Event::Relet(
            rent_id,
            renter,
            machine_id,
            gpu_num,
            add_duration,
            rent_fee,
        ));
        Ok(().into())
    }

    // 获取一个新的租用订单的ID
    pub fn get_new_rent_id() -> RentOrderId {
        let rent_id = Self::next_rent_id();

        let new_rent_id = loop {
            let new_rent_id = if rent_id == u64::MAX { 0 } else { rent_id + 1 };
            if !RentInfo::<T>::contains_key(new_rent_id) {
                break new_rent_id;
            }
        };

        NextRentId::<T>::put(new_rent_id);

        rent_id
    }

    // NOTE: 银河竞赛开启前，租金付给stash账户；开启后租金转到销毁账户
    // NOTE: 租金付给stash账户时，检查是否满足单卡10w/$300的质押条件，不满足，先质押.
    fn pay_rent_fee(
        renter: &T::AccountId,
        machine_id: MachineId,
        machine_stash: T::AccountId,
        fee_amount: BalanceOf<T>,
    ) -> DispatchResult {
        let rent_fee_pot = Self::rent_fee_pot().ok_or(Error::<T>::UndefinedRentPot)?;

        let destroy_percent = <online_profile::Pallet<T>>::rent_fee_destroy_percent();

        let fee_to_destroy = destroy_percent * fee_amount;
        let fee_to_stash = fee_amount.checked_sub(&fee_to_destroy).ok_or(Error::<T>::Overflow)?;

        <T as pallet::Config>::Currency::transfer(renter, &machine_stash, fee_to_stash, KeepAlive)?;
        <T as pallet::Config>::Currency::transfer(
            renter,
            &rent_fee_pot,
            fee_to_destroy,
            KeepAlive,
        )?;
        let _ = T::RTOps::change_machine_rent_fee(
            machine_stash,
            machine_id,
            fee_to_destroy,
            fee_to_stash,
        );
        Ok(())
    }

    // 定时检查机器是否30分钟没有上线
    fn check_machine_starting_status() -> Result<(), ()> {
        let now = <frame_system::Pallet<T>>::block_number();

        if !<ConfirmingOrder<T>>::contains_key(now) {
            return Ok(());
        }

        let pending_confirming = Self::confirming_order(now);
        for rent_id in pending_confirming {
            let rent_info = Self::rent_info(&rent_id).ok_or(())?;

            Self::clean_order(&rent_info.renter, rent_id)?;
            T::RTOps::change_machine_status_on_confirm_expired(
                &rent_info.machine_id,
                rent_info.gpu_num,
            )?;
        }
        Ok(())
    }

    // -Write: MachineRentOrder, RentEnding, RentOrder,
    // UserOrder, ConfirmingOrder
    fn clean_order(who: &T::AccountId, rent_order_id: RentOrderId) -> Result<(), ()> {
        let mut user_order = Self::user_order(who);
        ItemList::rm_item(&mut user_order, &rent_order_id);

        let rent_info = Self::rent_info(rent_order_id).ok_or(())?;

        // return back staked money!
        if !rent_info.stake_amount.is_zero() {
            let _ = Self::change_renter_total_stake(who, rent_info.stake_amount, false);
        }

        let mut rent_ending = Self::rent_ending(rent_info.rent_end);
        ItemList::rm_item(&mut rent_ending, &rent_order_id);

        let pending_confirming_deadline = rent_info.rent_start + WAITING_CONFIRMING_DELAY.into();
        let mut pending_confirming = Self::confirming_order(pending_confirming_deadline);
        ItemList::rm_item(&mut pending_confirming, &rent_order_id);

        let mut machine_rent_order = Self::machine_rent_order(&rent_info.machine_id);
        machine_rent_order.clean_expired_order(rent_order_id, rent_info.gpu_index);

        MachineRentOrder::<T>::insert(&rent_info.machine_id, machine_rent_order);
        if rent_ending.is_empty() {
            RentEnding::<T>::remove(rent_info.rent_end);
        } else {
            RentEnding::<T>::insert(rent_info.rent_end, rent_ending);
        }
        RentInfo::<T>::remove(rent_order_id);
        if user_order.is_empty() {
            UserOrder::<T>::remove(who);
        } else {
            UserOrder::<T>::insert(who, user_order);
        }
        if pending_confirming.is_empty() {
            ConfirmingOrder::<T>::remove(pending_confirming_deadline);
        } else {
            ConfirmingOrder::<T>::insert(pending_confirming_deadline, pending_confirming);
        }
        Ok(())
    }

    // - Write: UserTotalStake
    fn change_renter_total_stake(
        who: &T::AccountId,
        amount: BalanceOf<T>,
        is_add: bool,
    ) -> Result<(), ()> {
        let current_stake = Self::user_total_stake(who);

        let new_stake = if is_add {
            ensure!(<T as Config>::Currency::can_reserve(who, amount), ());
            <T as Config>::Currency::reserve(who, amount).map_err(|_| ())?;
            current_stake.checked_add(&amount).ok_or(())?
        } else {
            ensure!(current_stake >= amount, ());
            let _ = <T as Config>::Currency::unreserve(who, amount);
            current_stake.checked_sub(&amount).ok_or(())?
        };
        UserTotalStake::<T>::insert(who, new_stake);
        Ok(())
    }

    // 这里修rentMachine模块通知onlineProfile机器已经租用完成，
    // onlineProfile判断机器是否需要变成online状态，或者记录下之前是租用状态，
    // 以便机器再次上线时进行正确的惩罚
    fn check_if_rent_finished() -> Result<(), ()> {
        let now = <frame_system::Pallet<T>>::block_number();
        if !<RentEnding<T>>::contains_key(now) {
            return Ok(());
        }
        let pending_ending = Self::rent_ending(now);

        for rent_id in pending_ending {
            let rent_info = Self::rent_info(&rent_id).ok_or(())?;
            let machine_id = rent_info.machine_id.clone();
            let rent_duration = now.saturating_sub(rent_info.rent_start);

            // NOTE: 只要机器还有租用订单(租用订单>1)，就不修改成online状态。
            let is_last_rent = Self::is_last_rent(&machine_id, &rent_info.renter)?;
            let _ = T::RTOps::change_machine_status_on_rent_end(
                &machine_id,
                rent_info.gpu_num,
                rent_duration,
                is_last_rent.0,
                is_last_rent.1,
                rent_info.renter.clone(),
            );

            let _ = Self::clean_order(&rent_info.renter, rent_id);
        }
        Ok(())
    }

    // 当没有正在租用的机器时，可以修改得分快照
    // 判断machine_id的订单是否只有1个
    // 判断renter是否只租用了machine_id一次
    fn is_last_rent(machine_id: &MachineId, renter: &T::AccountId) -> Result<(bool, bool), ()> {
        let machine_order = Self::machine_rent_order(machine_id);
        let mut machine_order_count = 0;
        let mut renter_order_count = 0;

        // NOTE: 一定是正在租用的机器才算，正在确认中的租用不算
        for order_id in machine_order.rent_order {
            let rent_info = Self::rent_info(order_id).ok_or(())?;
            if renter == &rent_info.renter {
                renter_order_count = renter_order_count.saturating_add(1);
            }
            if matches!(rent_info.rent_status, RentStatus::Renting) {
                machine_order_count = machine_order_count.saturating_add(1);
            }
        }
        Ok((machine_order_count < 2, renter_order_count < 2))
    }
}
