// 用户租用逻辑
// 为了简化，该模块只提供最简单的租用情况： 整租，不能退租，
// 确认租用成功之后，将币转到一个特定地址，这个地址在后期稳定运行后burn掉自己的币

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
    dispatch::DispatchResult,
    ensure,
    pallet_prelude::*,
    traits::{
        Currency, ExistenceRequirement::KeepAlive, LockIdentifier, LockableCurrency,
        WithdrawReasons,
    },
    IterableStorageMap,
};
use frame_system::{ensure_root, ensure_signed, pallet_prelude::*};
pub use online_profile::{EraIndex, MachineId, MachineStatus};
use online_profile_machine::{DbcPrice, RTOps};
use sp_runtime::traits::{CheckedAdd, CheckedSub, SaturatedConversion};
use sp_std::{collections::btree_set::BTreeSet, prelude::*, str, vec::Vec};

type BalanceOf<T> =
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

pub const WAITING_CONFIRMING: u64 = 60; // 等待60个块，用户确认是否租用成功
pub const BLOCK_PER_DAY: u64 = 2880; // 1天按照2880个块
pub const DAY_PER_MONTH: u64 = 30; // 每个月30天计算租金
pub const CONFIRMING_DELAY: u64 = 60; // 租用之后60个块内确认机器租用成功

pub const PALLET_LOCK_ID: LockIdentifier = *b"rentmach";

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

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
            BalanceOf = BalanceOf<Self>,
        >;
        type DbcPrice: DbcPrice<BalanceOf = BalanceOf<Self>>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(_block_number: T::BlockNumber) {
            Self::check_machine_starting_status();
            if Self::check_if_rent_finished().is_err() {
                debug::error!("Check if rent is finished failed");
            }
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
    #[pallet::getter(fn rent_fee_pot)]
    pub(super) type RentFeePot<T: Config> = StorageValue<_, T::AccountId>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // 设置机器租金支付目标地址
        #[pallet::weight(0)]
        pub fn set_rent_fee_pot(
            origin: OriginFor<T>,
            pot_addr: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            RentFeePot::<T>::put(pot_addr);
            Ok(().into())
        }

        /// 用户租用机器
        #[pallet::weight(10000)]
        pub fn rent_machine(
            origin: OriginFor<T>,
            machine_id: MachineId,
            duration: EraIndex,
        ) -> DispatchResultWithPostInfo {
            let renter = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();
            let machine_info = <online_profile::Module<T>>::machines_info(&machine_id);

            // 用户提交订单，需要扣除10个DBC
            <generic_func::Module<T>>::pay_fixed_tx_fee(renter.clone())
                .map_err(|_| Error::<T>::PayTxFeeFailed)?;
            // 检查machine_id状态是否可以租用
            ensure!(
                machine_info.machine_status == MachineStatus::Online,
                Error::<T>::MachineNotRentable,
            );
            // 获得machine_price
            let machine_price = <online_profile::Module<T>>::calc_machine_price(
                machine_info.machine_info_detail.committee_upload_info.calc_point,
            )
            .ok_or(Error::<T>::GetMachinePriceFailed)?;
            let rent_fee_value =
                machine_price.checked_mul(duration as u64).ok_or(Error::<T>::Overflow)?;
            let rent_fee = <T as pallet::Config>::DbcPrice::get_dbc_amount_by_value(rent_fee_value)
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
                Some(renter.clone()),
                None,
            );
            PendingConfirming::<T>::insert(machine_id, renter);

            Ok(().into())
        }

        /// 用户在租用半小时(60个块)内确认机器租用成功
        #[pallet::weight(10000)]
        pub fn confirm_rent(
            origin: OriginFor<T>,
            machine_id: MachineId,
        ) -> DispatchResultWithPostInfo {
            let renter = ensure_signed(origin)?;
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

            Self::pay_rent_fee(
                &renter,
                machine_id.clone(),
                &machine_info.machine_stash,
                order_info.stake_amount,
            )?;

            order_info.confirm_rent = now;
            order_info.stake_amount = 0u64.saturated_into::<BalanceOf<T>>();
            order_info.rent_status = RentStatus::Renting;
            RentOrder::<T>::insert(&renter, &machine_id, order_info);

            // 改变online_profile状态
            T::RTOps::change_machine_status(
                &machine_id,
                MachineStatus::Rented,
                Some(renter.clone()),
                None,
            );
            PendingConfirming::<T>::remove(&machine_id);

            Self::deposit_event(Event::ConfirmRent(renter, machine_id));
            Ok(().into())
        }

        /// 用户续租
        #[pallet::weight(10000)]
        pub fn relet_machine(
            origin: OriginFor<T>,
            machine_id: MachineId,
            add_duration: EraIndex,
        ) -> DispatchResultWithPostInfo {
            let renter = ensure_signed(origin)?;

            let mut order_info =
                Self::rent_order(&renter, &machine_id).ok_or(Error::<T>::NoOrderExist)?;

            let machine_info = <online_profile::Module<T>>::machines_info(&machine_id);

            let machine_price = <online_profile::Module<T>>::calc_machine_price(
                machine_info.machine_info_detail.committee_upload_info.calc_point,
            )
            .ok_or(Error::<T>::GetMachinePriceFailed)?;
            let rent_fee_value =
                machine_price.checked_mul(add_duration as u64).ok_or(Error::<T>::Overflow)?;
            let rent_fee = <T as pallet::Config>::DbcPrice::get_dbc_amount_by_value(rent_fee_value)
                .ok_or(Error::<T>::Overflow)?;

            // 检查用户是否有足够的资金，来租用机器
            let user_balance = <T as pallet::Config>::Currency::free_balance(&renter);
            ensure!(rent_fee < user_balance, Error::<T>::InsufficientValue);

            Self::pay_rent_fee(&renter, machine_id.clone(), &machine_info.machine_stash, rent_fee)?;

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
        GetMachinePriceFailed,
    }
}

impl<T: Config> Pallet<T> {
    // NOTE: 银河竞赛开启前，租金付给stash账户；开启后租金转到销毁账户
    fn pay_rent_fee(
        renter: &T::AccountId,
        machine_id: MachineId,
        machine_stash: &T::AccountId,
        fee_amount: BalanceOf<T>,
    ) -> DispatchResult {
        let rent_fee_pot = Self::rent_fee_pot().ok_or(Error::<T>::UndefinedRentPot)?;
        let galaxy_is_on = <online_profile::Module<T>>::galaxy_is_on();
        if galaxy_is_on {
            <T as pallet::Config>::Currency::transfer(renter, &rent_fee_pot, fee_amount, KeepAlive)
                .map_err(|_| DispatchError::Other("Can't make tx payment"))?;

            T::RTOps::change_machine_rent_fee(fee_amount, machine_id.clone(), true);
        } else {
            <T as pallet::Config>::Currency::transfer(renter, machine_stash, fee_amount, KeepAlive)
                .map_err(|_| DispatchError::Other("Can't make tx payment"))?;

            T::RTOps::change_machine_rent_fee(fee_amount, machine_id, false);
        }
        Ok(())
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
            let duration = now.checked_sub(&rent_order.rent_start).unwrap_or(0u64.saturated_into());

            if duration > WAITING_CONFIRMING.saturated_into() {
                // 超过了60个块，也就是30分钟
                Self::clean_order(&renter, &machine_id);

                T::RTOps::change_machine_status(&machine_id, MachineStatus::Online, None, None);
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

            for a_machine in user_rented {
                let rent_info = Self::rent_order(&a_renter, &a_machine).ok_or(())?;
                if now > rent_info.rent_end {
                    let machine_info = <online_profile::Module<T>>::machines_info(&a_machine);
                    let mut machine_status = MachineStatus::Online;

                    if let MachineStatus::StakerReportOffline(offline_time, status) =
                        machine_info.machine_status
                    {
                        // 如果是rented状态，则改为online
                        if let MachineStatus::Rented = *status {
                            machine_status = MachineStatus::StakerReportOffline(
                                offline_time,
                                Box::new(MachineStatus::Online),
                            );
                        }
                    }

                    T::RTOps::change_machine_status(
                        &a_machine,
                        machine_status,
                        None,
                        Some((rent_info.rent_end - rent_info.rent_start).saturated_into()),
                    );

                    Self::clean_order(&a_renter, &a_machine);
                }
            }
        }
        Ok(())
    }
}

// RPC
impl<T: Config> Module<T> {
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
