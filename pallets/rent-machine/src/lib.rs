// 用户租用逻辑
// 为了简化，该模块只提供最简单的租用情况： 整租，不能退租，

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
    ensure,
    pallet_prelude::*,
    traits::{Currency, LockIdentifier, LockableCurrency, WithdrawReasons},
};
use frame_system::{ensure_root, ensure_signed, pallet_prelude::*};
use sp_runtime::{
    traits::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, SaturatedConversion},
    RuntimeDebug,
};
use sp_std::{prelude::*, str, vec::Vec};
use online_profile::MachineStatus;

type BalanceOf<T> =
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
pub type MachineId = Vec<u8>;
pub type EraIndex = u32;

pub const PALLET_LOCK_ID: LockIdentifier = *b"rentmach";

pub use pallet::*;

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, Default)]
pub struct RentOrderDetail<AccountId, BlockNumber, Balance> {
    pub renter: AccountId, // 租用者
    pub rent_start: BlockNumber, // 租用开始时间
    pub rent_end: BlockNumber, // 租用结束时间
    pub stake_amount: Balance, // 用户对该机器的质押
    pub task_id: Vec<u8>, // task_id
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + online_profile::Config + {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Currency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(_block_number: T::BlockNumber) {
            Self::check_machine_starting_status();
        }
    }

    // 存储用户当前租用的机器列表
    #[pallet::storage]
    #[pallet::getter(fn rent_machine_list)]
    pub(super) type RentMachineList<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, Vec<MachineId>, ValueQuery>;

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

    // 存储每个用户在该模块中的总质押量
    #[pallet::storage]
    #[pallet::getter(fn user_total_stake)]
    pub(super) type UserTotalStake<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // 用户租用机器
        #[pallet::weight(10000)]
        pub fn rent_machine(origin: OriginFor<T>, machine_id: MachineId, duration: EraIndex) -> DispatchResultWithPostInfo {
            let renter = ensure_signed(origin)?;

            // TODO: 用户提交订单，需要扣除10个DBC

            let now = <frame_system::Module<T>>::block_number();
            let machine_info = <online_profile::Module<T>>::machines_info(&machine_id);

            // 0. 检查machine_id是否可以租用
            if machine_info.machine_status != MachineStatus::Online {
                return Err(Error::<T>::MachineNotRentable.into())
            }

            // 获得machine_price
            let rent_fee = Self::stake_dbc_amount(machine_info.machine_price, duration).ok_or(Error::<T>::Overflow)?;

            // 检查用户是否有足够的资金，来租用机器
            let user_balance = <T as pallet::Config>::Currency::free_balance(&renter);
            ensure!(rent_fee < user_balance, Error::<T>::InsufficientValue);

            // 获取用户租用的结束时间
            let rent_end = 2880u64.checked_mul(duration as u64).ok_or(Error::<T>::Overflow)?
                .saturated_into::<T::BlockNumber>().checked_add(&now).ok_or(Error::<T>::Overflow)?;

            // 质押用户的资金，并修改机器状态
            Self::add_user_total_stake(&renter, rent_fee).map_err(|_| Error::<T>::InsufficientValue)?;

            RentOrder::<T>::insert(&renter, &machine_id, RentOrderDetail {
                renter: renter.clone(),
                rent_start: now,
                rent_end: rent_end,
                stake_amount: rent_fee,
                ..Default::default()
            });

            let mut user_rented = Self::rent_machine_list(&renter);
            if let Err(index) = user_rented.binary_search(&machine_id) {
                user_rented.insert(index, machine_id);
            }
            RentMachineList::<T>::insert(&renter, user_rented);

            // TODO: 改变online_profile状态，影响机器佣金

            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn confirm_rent(origin: OriginFor<T>, machine_id: MachineId, task_id: Vec<u8>) -> DispatchResultWithPostInfo {
            let renter = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            let mut order_info = Self::rent_order(&renter, &machine_id).ok_or(Error::<T>::NoOrderExist)?;

            // 不能超过30分钟
            let machine_start_duration = now.checked_sub(&order_info.rent_start).ok_or(Error::<T>::Overflow)?;
            if machine_start_duration.saturated_into::<u64>() > 60u64 {
                return Err(Error::<T>::ExpiredConfirm.into());
            }

            let machine_info = <online_profile::Module<T>>::machines_info(&machine_id);
            if machine_info.machine_status != MachineStatus::Creating {
                return Err(Error::<T>::StatusNotAllowed.into());
            }

            order_info.task_id = task_id;
            RentOrder::<T>::insert(&renter, &machine_id, order_info);

            // TODO: 改变online_profile状态
            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        StakeToBeCandidacy(T::AccountId, BalanceOf<T>),
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
    }
}

impl<T: Config> Pallet<T> {
    // 根据DBC价格获得需要质押数量
    fn stake_dbc_amount(machine_price: u64, rent_duration: EraIndex) -> Option<BalanceOf<T>> {
        let dbc_price: BalanceOf<T> = <dbc_price_ocw::Module<T>>::avg_price()?.saturated_into();
        let one_dbc: BalanceOf<T> = 1000_000_000_000_000u64.saturated_into();

        let renter_need: BalanceOf<T> = machine_price.checked_mul(rent_duration as u64)?.saturated_into();
        one_dbc.checked_mul(&renter_need)?.checked_div(&dbc_price)
    }

    // 定时检查机器是否30分钟没有上线
    fn check_machine_starting_status() {
        todo!();
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

        Ok(())
    }
}
