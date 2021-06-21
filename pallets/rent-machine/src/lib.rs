// 用户租用逻辑
// 为了简化，该模块只提供最简单的租用情况： 整租，不能退租，
// 确认租用成功之后，将币转到一个特定地址，这个地址在后期稳定运行后burn掉自己的币

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
    ensure,
    pallet_prelude::*,
    traits::{
        Currency, ExistenceRequirement::KeepAlive, LockIdentifier, LockableCurrency, OnUnbalanced,
        WithdrawReasons,
    },
    IterableStorageMap,
};
use frame_system::{ensure_root, ensure_signed, pallet_prelude::*};
pub use online_profile::{EraIndex, MachineId, MachineStatus};
use online_profile_machine::RTOps;
use sp_runtime::traits::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, SaturatedConversion};
use sp_std::{collections::btree_set::BTreeSet, prelude::*, str, vec::Vec};

type BalanceOf<T> =
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T> = <<T as Config>::Currency as Currency<
    <T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

pub const BLOCK_PER_DAY: u64 = 2880; // 1天按照2880个块
pub const DAY_PER_MONTH: u64 = 30; // 每个月30天计算租金
pub const CONFIRMING_DELAY: u64 = 60; // 租用之后60个块内确认机器租用成功

pub const PALLET_LOCK_ID: LockIdentifier = *b"rentmach";

pub use pallet::*;
mod rpc_types;
pub use rpc_types::RpcRentOrderDetail;

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, Default)]
pub struct RentOrderDetail<AccountId, BlockNumber, Balance> {
    pub renter: AccountId,         // 租用者
    pub rent_start: BlockNumber,   // 租用开始时间
    pub confirm_rent: BlockNumber, // 用户确认租成功的时间
    pub rent_end: BlockNumber,     // 租用结束时间
    pub stake_amount: Balance,     // 用户对该机器的质押
    pub rent_status: RentStatus,   // 当前订单的状态
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum RentStatus {
    WaitingVerifying,
    Renting,
    RentExpired,
}

impl Default for RentStatus {
    fn default() -> Self {
        RentStatus::WaitingVerifying
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + online_profile::Config + generic_func::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Currency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;
        type RTOps: RTOps<
            MachineId = MachineId,
            MachineStatus = MachineStatus<Self::BlockNumber>,
            AccountId = Self::AccountId,
        >;
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
    pub(super) type UserRented<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, Vec<MachineId>, ValueQuery>;

    // 用户当前租用的某个机器的详情
    #[pallet::storage]
    #[pallet::getter(fn rent_order)]
    pub(super) type RentOrder<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        MachineId,
        RentOrderDetail<T::AccountId, T::BlockNumber, BalanceOf<T>>,
    >;

    // 等待用户确认租用成功的机器
    #[pallet::storage]
    #[pallet::getter(fn pending_confirming)]
    pub(super) type PendingConfirming<T: Config> =
        StorageMap<_, Blake2_128Concat, MachineId, T::AccountId, ValueQuery>;

    // 存储每个用户在该模块中的总质押量
    #[pallet::storage]
    #[pallet::getter(fn user_total_stake)]
    pub(super) type UserTotalStake<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

    // TODO: 统一在generic模块中设置
    // 租金支付目标地址
    #[pallet::storage]
    #[pallet::getter(fn rent_pot)]
    pub(super) type RentPot<T: Config> = StorageValue<_, T::AccountId>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // 设置机器租金支付目标地址
        #[pallet::weight(0)]
        pub fn set_rent_pot(
            origin: OriginFor<T>,
            pot_addr: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            RentPot::<T>::put(pot_addr);
            Ok(().into())
        }

        // 用户租用机器
        #[pallet::weight(10000)]
        pub fn rent_machine(
            origin: OriginFor<T>,
            machine_id: MachineId,
            duration: EraIndex,
        ) -> DispatchResultWithPostInfo {
            let renter = ensure_signed(origin)?;

            // 用户提交订单，需要扣除10个DBC
            <generic_func::Module<T>>::pay_fixed_tx_fee(renter.clone())
                .map_err(|_| Error::<T>::PayTxFeeFailed)?;

            let now = <frame_system::Module<T>>::block_number();
            let machine_info = <online_profile::Module<T>>::machines_info(&machine_id);

            // 检查machine_id状态是否可以租用
            if machine_info.machine_status != MachineStatus::Online {
                return Err(Error::<T>::MachineNotRentable.into());
            }

            // 获得machine_price
            let rent_fee = Self::stake_dbc_amount(machine_info.machine_price, duration)
                .ok_or(Error::<T>::Overflow)?;

            // 检查用户是否有足够的资金，来租用机器
            let user_balance = <T as pallet::Config>::Currency::free_balance(&renter);
            ensure!(rent_fee < user_balance, Error::<T>::InsufficientValue);

            // 获取用户租用的结束时间
            let rent_end = BLOCK_PER_DAY
                .checked_mul(duration as u64)
                .ok_or(Error::<T>::Overflow)?
                .saturated_into::<T::BlockNumber>()
                .checked_add(&now)
                .ok_or(Error::<T>::Overflow)?;

            // 质押用户的资金，并修改机器状态
            Self::add_user_total_stake(&renter, rent_fee)
                .map_err(|_| Error::<T>::InsufficientValue)?;

            RentOrder::<T>::insert(
                &renter,
                &machine_id,
                RentOrderDetail {
                    renter: renter.clone(),
                    rent_start: now,
                    rent_end,
                    stake_amount: rent_fee,
                    ..Default::default()
                },
            );

            let mut user_rented = Self::user_rented(&renter);
            if let Err(index) = user_rented.binary_search(&machine_id) {
                user_rented.insert(index, machine_id.clone());
            }
            UserRented::<T>::insert(&renter, user_rented);

            // 改变online_profile状态，影响机器佣金
            T::RTOps::change_machine_status(
                &machine_id,
                MachineStatus::Creating,
                renter.clone(),
                true,
            );
            PendingConfirming::<T>::insert(machine_id, renter);

            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn confirm_rent(
            origin: OriginFor<T>,
            machine_id: MachineId,
        ) -> DispatchResultWithPostInfo {
            let renter = ensure_signed(origin)?;
            let rent_pot = Self::rent_pot().ok_or(Error::<T>::UndefinedRentPot)?;
            let now = <frame_system::Module<T>>::block_number();

            let mut order_info =
                Self::rent_order(&renter, &machine_id).ok_or(Error::<T>::NoOrderExist)?;

            // 不能超过30分钟
            let machine_start_duration =
                now.checked_sub(&order_info.rent_start).ok_or(Error::<T>::Overflow)?;
            if machine_start_duration.saturated_into::<u64>() > CONFIRMING_DELAY {
                return Err(Error::<T>::ExpiredConfirm.into());
            }

            let machine_info = <online_profile::Module<T>>::machines_info(&machine_id);
            if machine_info.machine_status != MachineStatus::Creating {
                return Err(Error::<T>::StatusNotAllowed.into());
            }

            // 质押转到特定账户
            Self::reduce_total_stake(&renter, order_info.stake_amount)
                .map_err(|_| Error::<T>::UnlockToPayFeeFailed)?;

            <T as pallet::Config>::Currency::transfer(
                &renter,
                &rent_pot,
                order_info.stake_amount,
                KeepAlive,
            )
            .map_err(|_| DispatchError::Other("Can't make tx payment"))?;

            order_info.confirm_rent = now;
            order_info.stake_amount = 0u64.saturated_into::<BalanceOf<T>>();
            order_info.rent_status = RentStatus::Renting;
            RentOrder::<T>::insert(&renter, &machine_id, order_info);

            // 改变online_profile状态
            T::RTOps::change_machine_status(
                &machine_id,
                MachineStatus::Rented,
                renter.clone(),
                true,
            );
            PendingConfirming::<T>::remove(&machine_id);

            Self::deposit_event(Event::ConfirmRent(renter, machine_id));
            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn add_rent(
            origin: OriginFor<T>,
            machine_id: MachineId,
            add_duration: EraIndex,
        ) -> DispatchResultWithPostInfo {
            let renter = ensure_signed(origin)?;
            let rent_pot = Self::rent_pot().ok_or(Error::<T>::UndefinedRentPot)?;

            let mut order_info =
                Self::rent_order(&renter, &machine_id).ok_or(Error::<T>::NoOrderExist)?;

            let machine_info = <online_profile::Module<T>>::machines_info(&machine_id);
            let rent_fee = Self::stake_dbc_amount(machine_info.machine_price, add_duration)
                .ok_or(Error::<T>::Overflow)?;

            // 检查用户是否有足够的资金，来租用机器
            let user_balance = <T as pallet::Config>::Currency::free_balance(&renter);
            ensure!(rent_fee < user_balance, Error::<T>::InsufficientValue);

            <T as pallet::Config>::Currency::transfer(&renter, &rent_pot, rent_fee, KeepAlive)
                .map_err(|_| DispatchError::Other("Can't make tx payment"))?;

            order_info.rent_end += add_duration.saturated_into::<T::BlockNumber>();
            RentOrder::<T>::insert(&renter, &machine_id, order_info);

            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        PayTxFee(T::AccountId, BalanceOf<T>),
        ConfirmRent(T::AccountId, MachineId),
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
    }
}

impl<T: Config> Pallet<T> {
    // 根据DBC价格计算租金
    // 租金 = 机器价格 * 租用era * one_dbc / dbc_price / 30 (天/月)
    fn stake_dbc_amount(machine_price: u64, rent_duration: EraIndex) -> Option<BalanceOf<T>> {
        let one_dbc: BalanceOf<T> = 1000_000_000_000_000u64.saturated_into();
        let dbc_price: BalanceOf<T> = <dbc_price_ocw::Module<T>>::avg_price()?.saturated_into();

        let renter_need: BalanceOf<T> =
            machine_price.checked_mul(rent_duration as u64)?.saturated_into();

        one_dbc
            .checked_mul(&renter_need)?
            .checked_div(&dbc_price)?
            .checked_div(&DAY_PER_MONTH.saturated_into::<BalanceOf<T>>())
    }

    // 定时检查机器是否30分钟没有上线
    fn check_machine_starting_status() {
        let pending_confirming = Self::pending_confirming_order();
        let now = <frame_system::Module<T>>::block_number();
        for (machine_id, renter) in pending_confirming {
            let rent_order = Self::rent_order(&renter, &machine_id);
            if let None = rent_order {
                continue;
            }
            let rent_order = rent_order.unwrap();
            let duration = now.checked_sub(&rent_order.rent_start);
            if let None = duration {
                debug::error!("Duration of confirming rent cannot be None");
                Self::clean_order(&renter, &machine_id);

                T::RTOps::change_machine_status(
                    &machine_id,
                    MachineStatus::Online,
                    renter.clone(),
                    false,
                );
                continue;
            }
            let duration = duration.unwrap();
            if duration > 60u64.saturated_into() {
                // 超过了60个块，也就是30分钟
                Self::clean_order(&renter, &machine_id);

                T::RTOps::change_machine_status(
                    &machine_id,
                    MachineStatus::Online,
                    renter.clone(),
                    false,
                );
                continue;
            }
        }
    }

    fn clean_order(who: &T::AccountId, machine_id: &MachineId) {
        let mut rent_machine_list = Self::user_rented(who);
        if let Ok(index) = rent_machine_list.binary_search(machine_id) {
            rent_machine_list.remove(index);
        }

        let rent_info = Self::rent_order(who, machine_id);
        if let Some(rent_info) = rent_info {
            // return back staked money!
            let _ = Self::reduce_total_stake(who, rent_info.stake_amount);
        }

        RentOrder::<T>::remove(who, machine_id);
        UserRented::<T>::insert(who, rent_machine_list);
        PendingConfirming::<T>::remove(machine_id);
    }

    fn pending_confirming_order() -> BTreeSet<(MachineId, T::AccountId)> {
        <PendingConfirming<T> as IterableStorageMap<MachineId, T::AccountId>>::iter()
            .map(|(machine, acct)| (machine, acct))
            .collect::<BTreeSet<_>>()
    }

    fn add_user_total_stake(controller: &T::AccountId, amount: BalanceOf<T>) -> Result<(), ()> {
        let current_stake = Self::user_total_stake(controller);
        let next_stake = current_stake.checked_add(&amount).ok_or(())?;
        <T as pallet::Config>::Currency::set_lock(
            PALLET_LOCK_ID,
            controller,
            next_stake,
            WithdrawReasons::all(),
        );

        UserTotalStake::<T>::insert(controller, next_stake);
        Ok(())
    }

    fn reduce_total_stake(controller: &T::AccountId, amount: BalanceOf<T>) -> Result<(), ()> {
        let current_stake = Self::user_total_stake(controller);
        let next_stake = current_stake.checked_sub(&amount).ok_or(())?;
        <T as pallet::Config>::Currency::set_lock(
            PALLET_LOCK_ID,
            controller,
            next_stake,
            WithdrawReasons::all(),
        );

        UserTotalStake::<T>::insert(controller, next_stake);
        Ok(())
    }

    fn get_all_renter() -> Vec<T::AccountId> {
        <UserRented<T> as IterableStorageMap<T::AccountId, _>>::iter()
            .map(|(renter, _)| renter)
            .collect::<Vec<_>>()
    }

    // 检查机器是否已经下线，如果下线则改变机器状态
    fn check_if_rent_finished() -> Result<(), ()> {
        let now = <frame_system::Module<T>>::block_number();

        let all_renter = Self::get_all_renter();
        for a_renter in all_renter {
            let user_rented = Self::user_rented(&a_renter);
            let mut rented_machine = user_rented.clone();

            for a_machine in user_rented {
                let mut rent_info = Self::rent_order(&a_renter, &a_machine).ok_or(())?;
                if rent_info.rent_end
                    > now + (BLOCK_PER_DAY * 10u64).saturated_into::<T::BlockNumber>()
                {
                    RentOrder::<T>::remove(&a_renter, &a_machine);
                    if let Ok(index) = rented_machine.binary_search(&a_machine) {
                        rented_machine.remove(index);
                    }
                } else if rent_info.rent_end > now {
                    rent_info.rent_status = RentStatus::RentExpired;
                    T::RTOps::change_machine_status(
                        &a_machine,
                        MachineStatus::Creating,
                        a_renter.clone(),
                        false,
                    );
                    RentOrder::<T>::insert(&a_renter, &a_machine, rent_info);
                }
            }

            UserRented::<T>::insert(&a_renter, rented_machine);
        }
        Ok(())
    }
}

// RPC
impl<T: Config> Module<T> {
    pub fn get_sum() -> u64 {
        3
    }

    pub fn get_rent_order(
        renter: T::AccountId,
        machine_id: MachineId,
    ) -> RpcRentOrderDetail<T::AccountId, T::BlockNumber, BalanceOf<T>> {
        let order_info = Self::rent_order(&renter, &machine_id);
        if let None = order_info {
            return RpcRentOrderDetail { ..Default::default() };
        }
        let order_info = order_info.unwrap();
        return RpcRentOrderDetail {
            renter: order_info.renter,
            rent_start: order_info.rent_start,
            confirm_rent: order_info.confirm_rent,
            rent_end: order_info.rent_end,
            stake_amount: order_info.stake_amount,
        };
    }

    pub fn get_rent_list(renter: T::AccountId) -> Vec<MachineId> {
        Self::user_rented(&renter)
    }
}
