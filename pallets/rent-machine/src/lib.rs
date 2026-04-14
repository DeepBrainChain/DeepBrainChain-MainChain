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
    rental_type::{MachineGPUOrder, MachineRenterRentedOrderDetail, RentOrderDetail, RentStatus},
    traits::{DbcPrice, MachineInfoTrait, RTOps},
    EraIndex, ItemList, MachineId, RentOrderId, HALF_HOUR, ONE_DAY, ONE_MINUTE,
};
use frame_support::{
    dispatch::DispatchResult,
    ensure,
    pallet_prelude::*,
    traits::{Currency, ExistenceRequirement::KeepAlive, ReservableCurrency},
};
use frame_system::{ensure_root, ensure_signed, pallet_prelude::*};
use sp_core::H160;
use sp_runtime::{
    traits::{CheckedAdd, CheckedSub, SaturatedConversion, Saturating, Zero},
    Perbill,
};
use sp_std::{prelude::*, str, vec::Vec};

type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

/// 等待15min，用户确认是否租用成功
pub const WAITING_CONFIRMING_DELAY: u32 = 15 * ONE_MINUTE;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + online_profile::Config {
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
        fn on_finalize(block_number: T::BlockNumber) {
            let _ = Self::check_machine_starting_status(block_number);
            let _ = Self::check_if_rent_finished(block_number);
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
    pub type MachineRentOrder<T: Config> =
        StorageMap<_, Blake2_128Concat, MachineId, MachineGPUOrder, ValueQuery>;

    //Vec(renter,rent_start,rent_end)
    #[pallet::storage]
    #[pallet::getter(fn machine_renter_rented_orders)]
    pub type MachineRenterRentedOrders<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        MachineId,
        Blake2_128Concat,
        T::AccountId,
        Vec<MachineRenterRentedOrderDetail<T::BlockNumber>>,
        ValueQuery,
    >;
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
    pub type RentEnding<T: Config> =
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

    #[pallet::storage]
    #[pallet::getter(fn evm_address_to_account)]
    pub(super) type EvmAddress2Account<T: Config> =
        StorageMap<_, Blake2_128Concat, H160, T::AccountId>;

    /// 卡主自定义额外加价（USD×10^6 per day per GPU），在系统自动定价基础上叠加
    #[pallet::storage]
    #[pallet::getter(fn machine_extra_price)]
    pub(super) type MachineExtraPrice<T: Config> =
        StorageMap<_, Blake2_128Concat, MachineId, u64, ValueQuery>;

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

        /// 卡主设置机器额外加价（在系统自动定价基础上叠加）
        /// 单位：USD×10^6 per day per GPU，与 get_machine_price 返回值单位一致
        /// 设置为 0 表示不额外加价
        #[pallet::call_index(10)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn set_machine_extra_price(
            origin: OriginFor<T>,
            machine_id: MachineId,
            extra_price: u64,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let machine_info = <online_profile::Pallet<T>>::machines_info(&machine_id)
                .ok_or(Error::<T>::Unknown)?;
            // 只有卡主(stash)或控制者(controller)可以设置
            ensure!(
                machine_info.machine_stash == who || machine_info.controller == who,
                Error::<T>::NoPermission
            );
            MachineExtraPrice::<T>::insert(&machine_id, extra_price);
            Self::deposit_event(Event::MachineExtraPriceSet(machine_id, extra_price));
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

            let confirming_order_block = rent_info.rent_start + WAITING_CONFIRMING_DELAY.into();
            let mut confirming_order = ConfirmingOrder::<T>::get(confirming_order_block);
            ItemList::rm_item(&mut confirming_order, &rent_id);
            if confirming_order.is_empty() {
                ConfirmingOrder::<T>::remove(confirming_order_block);
            } else {
                ConfirmingOrder::<T>::insert(confirming_order_block, confirming_order);
            }
            RentInfo::<T>::insert(&rent_id, rent_info.clone());

            MachineRenterRentedOrders::<T>::mutate(&machine_id, &renter, |details| {
                details.push(MachineRenterRentedOrderDetail {
                    rent_start: rent_info.rent_start,
                    rent_end: rent_info.rent_end,
                    rent_id: rent_id.clone(),
                });
            });
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

        #[pallet::call_index(4)]
        #[pallet::weight(Weight::from_parts(10000, 0))]
        pub fn bond_evm_address(
            origin: OriginFor<T>,
            machine_id: MachineId,
            evm_address: H160,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let machine_info = online_profile::Pallet::<T>::machines_info(machine_id)
                .ok_or(Error::<T>::MachineNotFound.as_str())?;
            ensure!(
                machine_info.controller == who || machine_info.machine_stash == who,
                Error::<T>::NotMachineOwner
            );
            EvmAddress2Account::<T>::insert(evm_address, who.clone());
            Self::deposit_event(Event::SetEvmAddress(evm_address, who));
            Ok(().into())
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

        SetEvmAddress(H160, T::AccountId),
        // machine_id, extra_price (USD×10^6 per day per GPU)
        MachineExtraPriceSet(MachineId, u64),
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

        NotMachineOwner,
        SignVerifiedFailed,
        MachineNotRented,
        MachineNotFound,
        MoreThanOneRenter,
        NoPermission,
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
            return Ok(().into())
        }

        // 检查还有空闲的GPU
        ensure!(rent_gpu_num + machine_rented_gpu <= gpu_num, Error::<T>::GPUNotEnough);

        // 租用必须是30min的整数倍
        ensure!(duration % HALF_HOUR.into() == Zero::zero(), Error::<T>::OnlyHalfHourAllowed);

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

        // 获得machine_price(每天的价格) = 系统自动定价 + 卡主额外加价
        // 根据租用GPU数量计算价格
        let system_price =
            T::RTOps::get_machine_price(machine_info.calc_point(), rent_gpu_num, gpu_num)
                .ok_or(Error::<T>::GetMachinePriceFailed)?;
        let extra_price_per_gpu = Self::machine_extra_price(&machine_id);
        let extra_price = extra_price_per_gpu
            .checked_mul(rent_gpu_num as u64)
            .unwrap_or(0);
        let machine_price = system_price.checked_add(extra_price).unwrap_or(system_price);

        // 根据租用时长计算rent_fee
        let rent_fee_value = machine_price
            .checked_mul(duration.saturated_into::<u64>())
            .ok_or(Error::<T>::Overflow)?
            .checked_div(ONE_DAY.into())
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
        ensure!(duration >= (10 * ONE_MINUTE).into(), Error::<T>::ReletTooShort);
        ensure!(rent_info.renter == renter, Error::<T>::NotMachineRenter);
        ensure!(rent_info.rent_status == RentStatus::Renting, Error::<T>::NoOrderExist);

        let machine_info =
            <online_profile::Pallet<T>>::machines_info(&machine_id).ok_or(Error::<T>::Unknown)?;
        let calc_point = machine_info.calc_point();

        // 确保租用时间不超过设定的限制，计算最多续费租用到
        let now = <frame_system::Pallet<T>>::block_number();
        // 最大结束块高为 今天租用开始的时间 + 60天
        // 60 days * 24 hour/day * 60 min/hour * 2 block/min
        let max_rent_end = now.checked_add(&(60 * ONE_DAY).into()).ok_or(Error::<T>::Overflow)?;
        let wanted_rent_end = old_rent_end + duration;

        // 计算实际可续租时间 (块高)
        let add_duration: T::BlockNumber = if max_rent_end >= wanted_rent_end {
            duration
        } else {
            max_rent_end.saturating_sub(old_rent_end)
        };

        if add_duration == 0u32.into() {
            return Ok(().into())
        }

        // 计算rent_fee = 系统自动定价 + 卡主额外加价
        let system_price =
            T::RTOps::get_machine_price(calc_point, gpu_num, machine_info.gpu_num())
                .ok_or(Error::<T>::GetMachinePriceFailed)?;
        let extra_price = Self::machine_extra_price(&machine_id)
            .checked_mul(gpu_num as u64).unwrap_or(0);
        let machine_price = system_price.checked_add(extra_price).unwrap_or(system_price);
        let rent_fee_value = machine_price
            .checked_mul(add_duration.saturated_into::<u64>())
            .ok_or(Error::<T>::Overflow)?
            .checked_div(ONE_DAY.into())
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

        let mut old_rent_ending = RentEnding::<T>::get(old_rent_end);
        ItemList::rm_item(&mut old_rent_ending, &rent_id);
        if old_rent_ending.is_empty() {
            RentEnding::<T>::remove(old_rent_end);
        } else {
            RentEnding::<T>::insert(old_rent_end, old_rent_ending);
        }
        RentEnding::<T>::mutate(rent_info.rent_end, |rent_ending| {
            ItemList::add_item(rent_ending, rent_id);
        });

        MachineRenterRentedOrders::<T>::mutate(&machine_id, &renter, |details| {
            details.push(MachineRenterRentedOrderDetail {
                rent_start: rent_info.rent_start,
                rent_end: rent_info.rent_end,
                rent_id: rent_id.clone(),
            });
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
                break new_rent_id
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
    fn check_machine_starting_status(block_number: T::BlockNumber) -> Result<(), ()> {
        if !<ConfirmingOrder<T>>::contains_key(block_number) {
            return Ok(())
        }

        let pending_confirming = Self::confirming_order(block_number);
        for rent_id in pending_confirming {
            let rent_info = Self::rent_info(&rent_id).ok_or(())?;

            // return back staked money!
            if !rent_info.stake_amount.is_zero() {
                let _ = Self::change_renter_total_stake(
                    &rent_info.renter,
                    rent_info.stake_amount,
                    false,
                );
            }

            let mut user_order = Self::user_order(&rent_info.renter);
            ItemList::rm_item(&mut user_order, &rent_id);
            if user_order.is_empty() {
                UserOrder::<T>::remove(&rent_info.renter);
            } else {
                UserOrder::<T>::insert(&rent_info.renter, user_order);
            }

            let mut confirming_order = Self::confirming_order(block_number);
            ItemList::rm_item(&mut confirming_order, &rent_id);
            if confirming_order.is_empty() {
                ConfirmingOrder::<T>::remove(block_number);
            } else {
                ConfirmingOrder::<T>::insert(block_number, confirming_order);
            }

            let mut rent_ending = Self::rent_ending(rent_info.rent_end);
            ItemList::rm_item(&mut rent_ending, &rent_id);
            if rent_ending.is_empty() {
                RentEnding::<T>::remove(rent_info.rent_end);
            } else {
                RentEnding::<T>::insert(rent_info.rent_end, rent_ending);
            }

            let mut machine_rent_order = Self::machine_rent_order(&rent_info.machine_id);
            machine_rent_order.clean_expired_order(rent_id, rent_info.gpu_index);
            MachineRentOrder::<T>::insert(&rent_info.machine_id, machine_rent_order);

            RentInfo::<T>::remove(rent_id);

            T::RTOps::change_machine_status_on_confirm_expired(
                &rent_info.machine_id,
                rent_info.gpu_num,
            )?;
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
    fn check_if_rent_finished(block_number: T::BlockNumber) -> Result<(), ()> {
        if !<RentEnding<T>>::contains_key(block_number) {
            return Ok(())
        }

        let pending_ending = Self::rent_ending(block_number);
        for rent_id in pending_ending {
            let rent_info = Self::rent_info(&rent_id).ok_or(())?;
            let machine_id = rent_info.machine_id.clone();
            let rent_duration = rent_info.rent_end.saturating_sub(rent_info.rent_start);

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

            // return back staked money!
            if !rent_info.stake_amount.is_zero() {
                let _ = Self::change_renter_total_stake(
                    &rent_info.renter,
                    rent_info.stake_amount,
                    false,
                );
            }

            let mut user_order = Self::user_order(&rent_info.renter);
            ItemList::rm_item(&mut user_order, &rent_id);
            if user_order.is_empty() {
                UserOrder::<T>::remove(&rent_info.renter);
            } else {
                UserOrder::<T>::insert(&rent_info.renter, user_order);
            }

            let mut rent_ending = Self::rent_ending(block_number);
            ItemList::rm_item(&mut rent_ending, &rent_id);
            if rent_ending.is_empty() {
                RentEnding::<T>::remove(block_number);
            } else {
                RentEnding::<T>::insert(block_number, rent_ending);
            }

            let mut machine_rent_order = Self::machine_rent_order(&rent_info.machine_id);
            machine_rent_order.clean_expired_order(rent_id, rent_info.gpu_index);
            MachineRentOrder::<T>::insert(&rent_info.machine_id, machine_rent_order);

            RentInfo::<T>::remove(rent_id);
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
    pub fn get_rent_ids(machine_id: MachineId, renter: &T::AccountId) -> Vec<RentOrderId> {
        let machine_orders = Self::machine_rent_order(machine_id);

        let mut rent_ids: Vec<RentOrderId> = Vec::new();
        for order_id in machine_orders.rent_order {
            if let Some(rent_info) = Self::rent_info(order_id) {
                if renter == &rent_info.renter && rent_info.rent_status == RentStatus::Renting {
                    rent_ids.push(order_id);
                }
            }
        }
        rent_ids
    }

    pub fn get_rent_id_of_renting_dbc_machine_by_owner(
        machine_id: &MachineId,
    ) -> Option<RentOrderId> {
        let machine_order = Self::machine_rent_order(machine_id.clone());
        if let Some(machine_info) = online_profile::Pallet::<T>::machines_info(machine_id) {
            if machine_order.rent_order.len() == 1 {
                let rent_id = machine_order.rent_order[0];
                if let Some(rent_info) = Self::rent_info(machine_order.rent_order[0]) {
                    if rent_info.rent_status == RentStatus::Renting &&
                        rent_info.renter == machine_info.controller
                    {
                        return Some(rent_id)
                    }
                }
            }
        };
        None
    }
}

impl<T: Config> MachineInfoTrait for Pallet<T> {
    type BlockNumber = T::BlockNumber;

    fn get_machine_calc_point(machine_id: MachineId) -> u64 {
        let machine_info_result = online_profile::Pallet::<T>::machines_info(machine_id);
        if let Some(machine_info) = machine_info_result {
            return machine_info.calc_point()
        }
        0
    }

    fn get_machine_cpu_rate(machine_id: MachineId) -> u64 {
        let machine_info_result = online_profile::Pallet::<T>::machines_info(machine_id);
        if let Some(machine_info) = machine_info_result {
            return machine_info.cpu_rate()
        }
        0
    }

    fn get_machine_gpu_type_and_mem(machine_id: MachineId) -> (Vec<u8>, u64) {
        let machine_info_result = online_profile::Pallet::<T>::machines_info(machine_id);
        if let Some(machine_info) = machine_info_result {
            return machine_info.gpu_type_and_mem()
        }
        (Vec::new(), 0)
    }

    fn get_machine_gpu_num(machine_id: MachineId) -> u64 {
        let machine_info_result = online_profile::Pallet::<T>::machines_info(machine_id);
        if let Some(machine_info) = machine_info_result {
            return machine_info.gpu_num() as u64
        }
        0
    }

    // get machine rent end block number by owner
    fn get_rent_end_at(
        machine_id: MachineId,
        rent_id: RentOrderId,
    ) -> Result<T::BlockNumber, &'static str> {
        let machine_info = online_profile::Pallet::<T>::machines_info(&machine_id)
            .ok_or(Error::<T>::MachineNotFound.as_str())?;

        let renter_controller = machine_info.controller;
        let renter_stash = machine_info.machine_stash;

        let rent_info = Self::rent_info(rent_id).ok_or(Error::<T>::MachineNotRented.as_str())?;

        if rent_info.machine_id != machine_id {
            return Err(Error::<T>::NotMachineRenter.as_str())
        }

        if rent_info.renter != renter_controller && rent_info.renter != renter_stash {
            return Err(Error::<T>::NotMachineRenter.as_str())
        }

        Ok(rent_info.rent_end)
    }

    fn is_machine_owner(machine_id: MachineId, evm_address: H160) -> Result<bool, &'static str> {
        let account = Self::evm_address_to_account(evm_address)
            .ok_or(Error::<T>::NotMachineOwner.as_str())?;

        let machine_info = online_profile::Pallet::<T>::machines_info(machine_id)
            .ok_or(Error::<T>::MachineNotFound.as_str())?;

        return Ok(machine_info.controller == account || machine_info.machine_stash == account)
    }

    fn get_usdt_machine_rent_fee(
        machine_id: MachineId,
        duration: T::BlockNumber,
        rent_gpu_num: u32,
    ) -> Result<u64, &'static str> {
        let machine_info = <online_profile::Pallet<T>>::machines_info(&machine_id)
            .ok_or(Error::<T>::Unknown.as_str())?;

        let system_price = T::RTOps::get_machine_price(
            machine_info.calc_point(),
            rent_gpu_num,
            machine_info.gpu_num(),
        )
        .ok_or(Error::<T>::GetMachinePriceFailed)?;
        let extra_price = Self::machine_extra_price(&machine_id)
            .checked_mul(rent_gpu_num as u64).unwrap_or(0);
        let machine_price = system_price.checked_add(extra_price).unwrap_or(system_price);

        // 根据租用时长计算rent_fee
        let rent_fee_value = machine_price
            .checked_mul(duration.saturated_into::<u64>())
            .ok_or(Error::<T>::Overflow)?
            .checked_div(ONE_DAY.into())
            .ok_or(Error::<T>::Overflow)?;
        Ok(rent_fee_value.saturated_into::<u64>())
    }
    fn get_dlc_machine_rent_fee(
        machine_id: MachineId,
        duration: T::BlockNumber,
        rent_gpu_num: u32,
    ) -> Result<u64, &'static str> {
        let machine_info = <online_profile::Pallet<T>>::machines_info(&machine_id)
            .ok_or(Error::<T>::Unknown.as_str())?;

        let system_price = T::RTOps::get_machine_price(
            machine_info.calc_point(),
            rent_gpu_num,
            machine_info.gpu_num(),
        )
        .ok_or(Error::<T>::GetMachinePriceFailed)?;
        let extra_price = Self::machine_extra_price(&machine_id)
            .checked_mul(rent_gpu_num as u64).unwrap_or(0);
        let machine_price = system_price.checked_add(extra_price).unwrap_or(system_price);

        // 根据租用时长计算rent_fee
        let rent_fee_value = machine_price
            .checked_mul(duration.saturated_into::<u64>())
            .ok_or(Error::<T>::Overflow)?
            .checked_div(ONE_DAY.into())
            .ok_or(Error::<T>::Overflow)?;
        let rent_fee_value =
            Perbill::from_rational(25u32, 100u32) * rent_fee_value + rent_fee_value;

        let rent_fee = <T as Config>::DbcPrice::get_dlc_amount_by_value(rent_fee_value)
            .ok_or(Error::<T>::Overflow)?;
        Ok(rent_fee.saturated_into::<u64>())
    }

    fn get_dlc_rent_fee_by_calc_point(
        calc_point: u64,
        duration: T::BlockNumber,
        rent_gpu_num: u32,
        total_gpu_num: u32,
    ) -> Result<u64, &'static str> {
        let machine_price = T::RTOps::get_machine_price(calc_point, rent_gpu_num, total_gpu_num)
            .ok_or(Error::<T>::GetMachinePriceFailed)?;

        let rent_fee_value = machine_price
            .checked_mul(duration.saturated_into::<u64>())
            .ok_or(Error::<T>::Overflow)?
            .checked_div(ONE_DAY.into())
            .ok_or(Error::<T>::Overflow)?;

        let rent_fee = <T as Config>::DbcPrice::get_dlc_amount_by_value(rent_fee_value)
            .ok_or(Error::<T>::Overflow)?;
        Ok(rent_fee.saturated_into::<u64>())
    }

    fn get_dbc_machine_rent_fee(
        machine_id: MachineId,
        duration: T::BlockNumber,
        rent_gpu_num: u32,
    ) -> Result<u64, &'static str> {
        let machine_info = <online_profile::Pallet<T>>::machines_info(&machine_id)
            .ok_or(Error::<T>::Unknown.as_str())?;

        let system_price = T::RTOps::get_machine_price(
            machine_info.calc_point(),
            rent_gpu_num,
            machine_info.gpu_num(),
        )
        .ok_or(Error::<T>::GetMachinePriceFailed)?;
        let extra_price = Self::machine_extra_price(&machine_id)
            .checked_mul(rent_gpu_num as u64).unwrap_or(0);
        let machine_price = system_price.checked_add(extra_price).unwrap_or(system_price);

        // 根据租用时长计算rent_fee
        let rent_fee_value = machine_price
            .checked_mul(duration.saturated_into::<u64>())
            .ok_or(Error::<T>::Overflow)?
            .checked_div(ONE_DAY.into())
            .ok_or(Error::<T>::Overflow)?;
        let rent_fee = <T as Config>::DbcPrice::get_dbc_amount_by_value(rent_fee_value)
            .ok_or(Error::<T>::Overflow)?;
        Ok(rent_fee.saturated_into::<u64>())
    }
}
