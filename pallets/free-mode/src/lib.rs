#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(unused_crate_dependencies)]


// use parity_scale_codec::{Decode, Encode};
// use scale_info::TypeInfo;
// use std::option::Option;

// pub mod migrations;
// mod rpc;

// #[cfg(test)]
// mod mock;
// #[allow(non_upper_case_globals)]
// #[cfg(test)]
// mod tests;

pub use dbc_support::machine_type::MachineStatus;
use dbc_support::{
    rental_type::{MachineGPUOrder, MachineRenterRentedOrderDetail,ReportItem, RentOrderDetail, RentStatus},
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
use sp_runtime::{traits::{CheckedAdd, CheckedMul, CheckedSub, SaturatedConversion, Saturating, Zero}, Perbill};

use sp_std::{prelude::*, str, vec::Vec};

type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

/// 等待15min，用户确认是否租用成功
pub const WAITING_CONFIRMING_DELAY: u32 = 15 * ONE_MINUTE;

pub const AMOUNT_DEPOSIT_FREE_MODE :u32= 1000;
pub const AMOUNT_DEPOSIT_REPORT :u32 = 1000;
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    // use frame_support::{Deserialize, Serialize};
    // use frame_system::Account;
    // use sp_core::serde::de::Unexpected::Option;
    // use sp_runtime::biguint;
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
    //
    // #[pallet::hooks]
    // impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
    //     fn on_finalize(_block_number: T::BlockNumber) {
    //         let _ = Self::check_machine_starting_status();
    //         let _ = Self::check_if_rent_finished();
    //     }
    //
    //     // fn on_runtime_upgrade() -> Weight {
    //     //     frame_support::debug::RuntimeLogger::init();
    //     //     frame_support::debug::info!("🔍️ OnlineProfile Storage Migration start");
    //     //     let weight1 = online_profile::migrations::apply::<T>();
    //     //     frame_support::debug::info!("🚀 OnlineProfile Storage Migration end");
    //
    //     //     frame_support::debug::RuntimeLogger::init();
    //     //     frame_support::debug::info!("🔍️ RentMachine Storage Migration start");
    //     //     let weight2 = migrations::apply::<T>();
    //     //     frame_support::debug::info!("🚀 RentMachine Storage Migration end");
    //     //     weight1 + weight2
    //     // }
    // }

    // machine中设置为free mode的gpu的数量,如果大于0就是free mode
    #[pallet::storage]
    #[pallet::getter(fn free_mode_gpu_count_in_machine)]
    pub(super) type FreeModeGpuCountInMachine<T: Config> =
        StorageMap<_, Blake2_128Concat, MachineId, u32, ValueQuery>;



    #[pallet::storage]
    #[pallet::getter(fn reported_idx)]
    pub(super) type ReportedIdx<T: Config> =
        StorageValue<_, u32, ValueQuery>;

    
    #[pallet::storage]
    #[pallet::getter(fn report_items)]
    pub(super) type ReportItems<T: Config> =
    StorageMap<_, Blake2_128Concat, u32, ReportItem,ValueQuery>;
    

    #[pallet::storage]
    #[pallet::getter(fn notary_addr)]
    pub(super) type NotaryAddr<T: Config> =
    StorageMap<_, Blake2_128Concat, T::AccountId,bool, ValueQuery>;
    

    #[pallet::storage]
    #[pallet::getter(fn is_report_verified)]
    pub(super) type IsReportVerified<T: Config> =
    StorageDoubleMap<
        _,
        Blake2_128Concat,
        u32,
        Blake2_128Concat,
        T::AccountId,
        bool,
        ValueQuery,
    >;
    // StorageDoubleMap<_, Blake2_128Concat, T::AccountId,bool, ValueQuery>;
    

    #[pallet::storage]
    #[pallet::getter(fn amount_staked_for_report)]
    pub(super) type AmountStakedForReport<T: Config> =
    StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn is_reported)]
    pub(super) type IsReported<T: Config> =
    StorageMap<_, Blake2_128Concat, MachineId, bool, ValueQuery>;
    //
    // // 存储用户当前租用的机器列表
    // #[pallet::storage]
    // #[pallet::getter(fn user_order)]
    // pub(super) type UserOrder<T: Config> =
    //     StorageMap<_, Blake2_128Concat, T::AccountId, Vec<RentOrderId>, ValueQuery>;
    //
    // #[pallet::storage]
    // #[pallet::getter(fn machine_rent_order)]
    // pub type MachineRentOrder<T: Config> =
    //     StorageMap<_, Blake2_128Concat, MachineId, MachineGPUOrder, ValueQuery>;
    //
    // //Vec(renter,rent_start,rent_end)
    // #[pallet::storage]
    // #[pallet::getter(fn machine_renter_rented_orders)]
    // pub type MachineRenterRentedOrders<T: Config> = StorageDoubleMap<
    //     _,
    //     Blake2_128Concat,
    //     MachineId,
    //     Blake2_128Concat,
    //     T::AccountId,
    //     Vec<MachineRenterRentedOrderDetail<T::BlockNumber>>,
    //     ValueQuery,
    // >;
    // #[pallet::storage]
    // #[pallet::getter(fn next_rent_id)]
    // pub(super) type NextRentId<T: Config> = StorageValue<_, RentOrderId, ValueQuery>;
    //
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

    // // 等待用户确认租用成功的机器
    // #[pallet::storage]
    // #[pallet::getter(fn confirming_order)]
    // pub type ConfirmingOrder<T: Config> =
    //     StorageMap<_, Blake2_128Concat, T::BlockNumber, Vec<RentOrderId>, ValueQuery>;
    //
    // // 记录每个区块将要结束租用的机器
    // #[pallet::storage]
    // #[pallet::getter(fn rent_ending)]
    // pub(super) type RentEnding<T: Config> =
    //     StorageMap<_, Blake2_128Concat, T::BlockNumber, Vec<RentOrderId>, ValueQuery>;
    //
    // 存储每个用户在该模块中的总质押量
    #[pallet::storage]
    #[pallet::getter(fn user_total_stake)]
    pub(super) type UserTotalStake<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

    //
    //       //存储用户举报
    // #[pallet::storage]
    // #[pallet::getter(fn is_report_user)]
    // pub(super) type IsReportUser<T: Config> =
    //     StorageMap<_, Blake2_128Concat, T::AccountId, bool, ValueQuery>;
    //
    // // 租金支付目标地址
    // #[pallet::storage]
    // #[pallet::getter(fn rent_fee_pot)]
    // pub(super) type RentFeePot<T: Config> = StorageValue<_, T::AccountId>;
    //
    // #[pallet::type_value]
    // pub(super) fn MaximumRentalDurationDefault<T: Config>() -> EraIndex {
    //     60
    // }
    //
    // // 最大租用/续租用时间
    // #[pallet::storage]
    // #[pallet::getter(fn maximum_rental_duration)]
    // pub(super) type MaximumRentalDuration<T: Config> =
    //     StorageValue<_, EraIndex, ValueQuery, MaximumRentalDurationDefault<T>>;
    //
    // #[pallet::storage]
    // #[pallet::getter(fn evm_address_to_account)]
    // pub(super) type EvmAddress2Account<T: Config> =
    //     StorageMap<_, Blake2_128Concat, H160, T::AccountId>;
    //
    // // The current storage version.
    // #[pallet::storage]
    // #[pallet::getter(fn storage_version)]
    // pub(super) type StorageVersion<T: Config> = StorageValue<_, u16, ValueQuery>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // 设置机器租金支付目标地址
        // #[pallet::call_index(0)]
        // #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        // pub fn set_rent_fee_pot(
        //     origin: OriginFor<T>,
        //     pot_addr: T::AccountId,
        // ) -> DispatchResultWithPostInfo {
        //     ensure_root(origin)?;
        //     RentFeePot::<T>::put(pot_addr);
        //     Ok(().into())
        // }

        // /// 用户租用机器(按天租用)
        // #[pallet::call_index(1)]
        // #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        // pub fn rent_machine(
        //     origin: OriginFor<T>,
        //     machine_id: MachineId,
        //     rent_gpu_num: u32,
        //     duration: T::BlockNumber,
        // ) -> DispatchResultWithPostInfo {
        //     let renter = ensure_signed(origin)?;
        //     Self::rent_machine_by_block(renter, machine_id, rent_gpu_num, duration)
        // }

             /// 加入自由模式
             #[pallet::call_index(111)]
             #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
             pub fn join_free_mode(
                 origin: OriginFor<T>,
                 machine_id: MachineId,
                 rent_gpu_num: u32
             ) -> DispatchResultWithPostInfo {
                 let renter = ensure_signed(origin)?;
                 Self::join_free_mode_config(renter, machine_id, rent_gpu_num)
             }

            /// 退出自由模式
            #[pallet::call_index(112)]
            #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
            pub fn exit_free_mode(
                origin: OriginFor<T>,
                machine_id: MachineId,
                rent_gpu_num: u32
            ) -> DispatchResultWithPostInfo {
                let renter = ensure_signed(origin)?;
                Self::exit_free_mode_base(renter, machine_id, rent_gpu_num)
            }



            /// 举报机器
            #[pallet::call_index(113)]
            #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
            pub fn report(
                origin: OriginFor<T>,
                machine_id: MachineId,
            ) -> DispatchResultWithPostInfo {

                let renter = ensure_signed(origin)?;
                Self::report_base(renter, machine_id)


            }
/// 设置举报验证人
            #[pallet::call_index(114)]
            #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
            pub fn set_notary_addr (
                origin: OriginFor<T>,
                pot_addr: T::AccountId,
                value :bool,
            ) -> DispatchResultWithPostInfo {
                ensure_root(origin)?;
                Self::update_notary_addr(pot_addr,value)
                
            }
    /// 验证人赞成或反对这个举报事件
            #[pallet::call_index(115)]
            #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
            pub fn verify_report_item(
                origin: OriginFor<T>,
                id:u32,
                is_approve:bool,
            ) -> DispatchResultWithPostInfo {
                let who = ensure_signed(origin)?;
                Self::verify_report(who,id,is_approve)
            }
    

        // /// 用户在租用15min(30个块)内确认机器租用成功
        // #[pallet::call_index(2)]
        // #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        // pub fn confirm_rent(
        //     origin: OriginFor<T>,
        //     rent_id: RentOrderId,
        // ) -> DispatchResultWithPostInfo {
        //     let renter = ensure_signed(origin)?;
        //     let now = <frame_system::Pallet<T>>::block_number();
        //
        //     let mut rent_info = Self::rent_info(&rent_id).ok_or(Error::<T>::Unknown)?;
        //     let machine_id = rent_info.machine_id.clone();
        //     let gpu_num = rent_info.gpu_num.clone();
        //     ensure!(rent_info.renter == renter, Error::<T>::NoOrderExist);
        //     ensure!(
        //         rent_info.rent_status == RentStatus::WaitingVerifying,
        //         Error::<T>::NoOrderExist
        //     );
        //
        //     // 不能超过15分钟
        //     let machine_start_duration =
        //         now.checked_sub(&rent_info.rent_start).ok_or(Error::<T>::Overflow)?;
        //     ensure!(
        //         machine_start_duration <= WAITING_CONFIRMING_DELAY.into(),
        //         Error::<T>::ExpiredConfirm
        //     );
        //
        //     let machine_info = <online_profile::Pallet<T>>::machines_info(&machine_id)
        //         .ok_or(Error::<T>::Unknown)?;
        //     ensure!(
        //         machine_info.machine_status == MachineStatus::Rented,
        //         Error::<T>::StatusNotAllowed
        //     );
        //
        //     // 质押转到特定账户
        //     Self::change_renter_total_stake(&renter, rent_info.stake_amount, false)
        //         .map_err(|_| Error::<T>::UnlockToPayFeeFailed)?;
        //
        //     Self::pay_rent_fee(
        //         &renter,
        //         machine_id.clone(),
        //         machine_info.machine_stash,
        //         rent_info.stake_amount,
        //     )?;
        //
        //     // 在stake_amount设置0前记录，用作事件
        //     let rent_fee = rent_info.stake_amount;
        //     let rent_duration = rent_info.rent_end.saturating_sub(rent_info.rent_start);
        //
        //     rent_info.confirm_rent(now);
        //     rent_info.stake_amount = Default::default();
        //
        //     // 改变online_profile状态
        //     T::RTOps::change_machine_status_on_confirmed(&machine_id, renter.clone())
        //         .map_err(|_| Error::<T>::Unknown)?;
        //
        //     ConfirmingOrder::<T>::mutate(
        //         rent_info.rent_start + WAITING_CONFIRMING_DELAY.into(),
        //         |pending_confirming| {
        //             ItemList::rm_item(pending_confirming, &rent_id);
        //         },
        //     );
        //     RentInfo::<T>::insert(&rent_id, rent_info.clone());
        //
        //     MachineRenterRentedOrders::<T>::mutate(&machine_id, &renter, |details| {
        //         details.push(MachineRenterRentedOrderDetail {
        //             rent_start: rent_info.rent_start,
        //             rent_end: rent_info.rent_end,
        //             rent_id: rent_id.clone(),
        //         });
        //     });
        //     RentInfo::<T>::insert(&rent_id, rent_info);
        //
        //     Self::deposit_event(Event::ConfirmRent(
        //         rent_id,
        //         renter,
        //         machine_id,
        //         gpu_num,
        //         rent_duration,
        //         rent_fee,
        //     ));
        //     Ok(().into())
        // }
        //
        // /// 用户续租(按天续租), 通过order_id来续租
        // #[pallet::call_index(3)]
        // #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        // pub fn relet_machine(
        //     origin: OriginFor<T>,
        //     rent_id: RentOrderId,
        //     relet_duration: T::BlockNumber,
        // ) -> DispatchResultWithPostInfo {
        //     let renter = ensure_signed(origin)?;
        //     Self::relet_machine_by_block(renter, rent_id, relet_duration)
        // }

        // #[pallet::call_index(4)]
        // #[pallet::weight(Weight::from_parts(10000, 0))]
        // pub fn bond_evm_address(
        //     origin: OriginFor<T>,
        //     machine_id: MachineId,
        //     evm_address: H160,
        // ) -> DispatchResultWithPostInfo {
        //
        //     let who = ensure_signed(origin)?;
        //
        //     let machine_info = online_profile::Pallet::<T>::machines_info(machine_id)
        //         .ok_or(Error::<T>::MachineNotFound.as_str())?;
        //     ensure!(
        //         machine_info.controller == who || machine_info.machine_stash == who,
        //         Error::<T>::NotMachineOwner
        //     );
        //     EvmAddress2Account::<T>::insert(evm_address, who.clone());
        //     Self::deposit_event(Event::SetEvmAddress(evm_address, who));
        //     Ok(().into())
        // }
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
        JoinFreeMode(T::AccountId,MachineId,u32),
        ExitFreeMode(T::AccountId,MachineId,u32),
        ReportFreeMode(T::AccountId,MachineId,u32),
        UpdateNotaryAddr(T::AccountId,bool),
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
        NoMoreGpu,
    IsReported,
        NotFreeMode,
        Verified,
    }
}

impl<T: Config> Pallet<T> {
    //
    // fn rent_machine_by_block(
    //     renter: T::AccountId,
    //     machine_id: MachineId,
    //     rent_gpu_num: u32,
    //     duration: T::BlockNumber,
    // ) -> DispatchResultWithPostInfo {
    //
    //     let now = <frame_system::Pallet<T>>::block_number();
    //     let machine_info =
    //         <online_profile::Pallet<T>>::machines_info(&machine_id).ok_or(Error::<T>::Unknown)?;
    //     let machine_rented_gpu = <online_profile::Pallet<T>>::machine_rented_gpu(&machine_id);
    //     let gpu_num = machine_info.gpu_num();
    //
    //     if gpu_num == 0 || duration == Zero::zero() {
    //         return Ok(().into())
    //     }
    //
    //     // 检查还有空闲的GPU
    //     ensure!(rent_gpu_num + machine_rented_gpu <= gpu_num, Error::<T>::GPUNotEnough);
    //
    //     // 租用必须是30min的整数倍
    //     ensure!(duration % HALF_HOUR.into() == Zero::zero(), Error::<T>::OnlyHalfHourAllowed);
    //
    //     // 检查machine_id状态是否可以租用
    //     ensure!(
    //         machine_info.machine_status == MachineStatus::Online ||
    //             machine_info.machine_status == MachineStatus::Rented,
    //         Error::<T>::MachineNotRentable
    //     );
    //
    //     // 最大租用时间限制MaximumRentalDuration
    //     let duration =
    //         duration.min((Self::maximum_rental_duration().saturating_mul(ONE_DAY)).into());
    //
    //     // NOTE: 用户提交订单，需要扣除10个DBC
    //     <generic_func::Pallet<T>>::pay_fixed_tx_fee(renter.clone())
    //         .map_err(|_| Error::<T>::PayTxFeeFailed)?;
    //
    //     // 获得machine_price(每天的价格)
    //     // 根据租用GPU数量计算价格
    //     let machine_price =
    //         T::RTOps::get_machine_price(machine_info.calc_point(), rent_gpu_num, gpu_num)
    //             .ok_or(Error::<T>::GetMachinePriceFailed)?;
    //
    //     // 根据租用时长计算rent_fee
    //     let rent_fee_value = machine_price
    //         .checked_mul(duration.saturated_into::<u64>())
    //         .ok_or(Error::<T>::Overflow)?
    //         .checked_div(ONE_DAY.into())
    //         .ok_or(Error::<T>::Overflow)?;
    //     let rent_fee = <T as Config>::DbcPrice::get_dbc_amount_by_value(rent_fee_value)
    //         .ok_or(Error::<T>::Overflow)?;
    //
    //     // 获取用户租用的结束时间(块高)
    //     let rent_end = duration.checked_add(&now).ok_or(Error::<T>::Overflow)?;
    //
    //     // 质押用户的资金，并修改机器状态
    //     Self::change_renter_total_stake(&renter, rent_fee, true)
    //         .map_err(|_| Error::<T>::InsufficientValue)?;
    //
    //     let rent_id = Self::get_new_rent_id();
    //
    //     let mut machine_rent_order = Self::machine_rent_order(&machine_id);
    //     let rentable_gpu_index = machine_rent_order.gen_rentable_gpu(rent_gpu_num, gpu_num);
    //     ItemList::add_item(&mut machine_rent_order.rent_order, rent_id);
    //
    //     // 改变online_profile状态，影响机器佣金
    //     T::RTOps::change_machine_status_on_rent_start(&machine_id, rent_gpu_num)
    //         .map_err(|_| Error::<T>::Unknown)?;
    //
    //     RentInfo::<T>::insert(
    //         &rent_id,
    //         RentOrderDetail::new(
    //             machine_id.clone(),
    //             renter.clone(),
    //             now,
    //             rent_end,
    //             rent_fee,
    //             rent_gpu_num,
    //             rentable_gpu_index,
    //         ),
    //     );
    //
    //     UserOrder::<T>::mutate(&renter, |user_order| {
    //         ItemList::add_item(user_order, rent_id);
    //     });
    //
    //     RentEnding::<T>::mutate(rent_end, |rent_ending| {
    //         ItemList::add_item(rent_ending, rent_id);
    //     });
    //
    //     ConfirmingOrder::<T>::mutate(now + WAITING_CONFIRMING_DELAY.into(), |pending_confirming| {
    //         ItemList::add_item(pending_confirming, rent_id);
    //     });
    //
    //     MachineRentOrder::<T>::insert(&machine_id, machine_rent_order);
    //
    //     Self::deposit_event(Event::Rent(
    //         rent_id,
    //         renter,
    //         machine_id,
    //         rent_gpu_num,
    //         duration.into(),
    //         rent_fee,
    //     ));
    //     Ok(().into())
    // }



    fn join_free_mode_config(
        renter: T::AccountId,
        machine_id: MachineId,
        rent_gpu_num: u32,

    ) -> DispatchResultWithPostInfo {
        
        let now = <frame_system::Pallet<T>>::block_number();
        let machine_info =
            <online_profile::Pallet<T>>::machines_info(&machine_id).ok_or(Error::<T>::Unknown)?;
        // let machine_rented_gpu = <online_profile::Pallet<T>>::machine_rented_gpu(&machine_id);
        let gpu_num = machine_info.gpu_num();
       let gpu_free_count =  Self::free_mode_gpu_count_in_machine(&machine_id);
        ensure!(gpu_num - gpu_free_count >= rent_gpu_num ,Error::<T>::NoMoreGpu);

        // if gpu_num == 0 || duration == Zero::zero() {
        //     return Ok(().into())
        // }

        // 检查还有空闲的GPU
        // ensure!(rent_gpu_num + machine_rented_gpu <= gpu_num, Error::<T>::GPUNotEnough);

        // 租用必须是30min的整数倍
        // ensure!(duration % HALF_HOUR.into() == Zero::zero(), Error::<T>::OnlyHalfHourAllowed);

        // 检查machine_id状态是否可以租用
        // ensure!(
        //     machine_info.machine_status == MachineStatus::Online ||
        //         machine_info.machine_status == MachineStatus::Rented,
        //     Error::<T>::MachineNotRentable
        // );

        // 最大租用时间限制MaximumRentalDuration
        // let duration =
        //     duration.min((Self::maximum_rental_duration().saturating_mul(ONE_DAY)).into());

        // NOTE: 用户提交订单，每个gpu支付1000个dbc
        // <generic_func::Pallet<T>>::pay_fixed_tx_fee(renter.clone())
        //     .map_err(|_| Error::<T>::PayTxFeeFailed)?;

        // 获得machine_price(每天的价格)
        // 根据租用GPU数量计算价格
        // let machine_price =
        //     T::RTOps::get_machine_price(machine_info.calc_point(), rent_gpu_num, gpu_num)
        //         .ok_or(Error::<T>::GetMachinePriceFailed)?;

        // // 根据租用时长计算rent_fee
        // let rent_fee_value = machine_price
        //     .checked_mul(duration.saturated_into::<u64>())
        //     .ok_or(Error::<T>::Overflow)?
        //     .checked_div(ONE_DAY.into())
        //     .ok_or(Error::<T>::Overflow)?;
        // let rent_fee = <T as Config>::DbcPrice::get_dbc_amount_by_value(rent_fee_value)
        //     .ok_or(Error::<T>::Overflow)?;

        // // 获取用户租用的结束时间(块高)
        // let rent_end = duration.checked_add(&now).ok_or(Error::<T>::Overflow)?;

        let amount_stake =BalanceOf::<T>::from(gpu_free_count).checked_mul(&BalanceOf::<T>::from(AMOUNT_DEPOSIT_FREE_MODE)).ok_or(Error::<T>::Overflow)?;
        // 质押用户的资金，并修改机器状态
        Self::update_stake_amount_for_free_mode(&renter, amount_stake, true)
            .map_err(|_| Error::<T>::InsufficientValue)?;

        // let rent_id = Self::get_new_rent_id();

        // let mut machine_rent_order = Self::machine_rent_order(&machine_id);
        // let rentable_gpu_index = machine_rent_order.gen_rentable_gpu(rent_gpu_num, gpu_num);
        // ItemList::add_item(&mut machine_rent_order.rent_order, rent_id);

        // // 改变online_profile状态，影响机器佣金
        // T::RTOps::change_machine_status_on_rent_start(&machine_id, rent_gpu_num)
        //     .map_err(|_| Error::<T>::Unknown)?;

        // RentInfo::<T>::insert(
        //     &rent_id,
        //     RentOrderDetail::new(
        //         machine_id.clone(),
        //         renter.clone(),
        //         now,
        //         rent_end,
        //         rent_fee,
        //         rent_gpu_num,
        //         rentable_gpu_index,
        //     ),
        // );

        // UserOrder::<T>::mutate(&renter, |user_order| {
        //     ItemList::add_item(user_order, rent_id);
        // });

        // RentEnding::<T>::mutate(rent_end, |rent_ending| {
        //     ItemList::add_item(rent_ending, rent_id);
        // });

        // ConfirmingOrder::<T>::mutate(now + WAITING_CONFIRMING_DELAY.into(), |pending_confirming| {
        //     ItemList::add_item(pending_confirming, rent_id);
        // });

        // MachineRentOrder::<T>::insert(&machine_id, machine_rent_order);

        Self::deposit_event(Event::JoinFreeMode(
            renter, machine_id, rent_gpu_num
        ));
        Ok(().into())
    }


    fn exit_free_mode_base(
        renter: T::AccountId,
        machine_id: MachineId,
        rent_gpu_num: u32,

    ) -> DispatchResultWithPostInfo {

let is_repotted =         Self::is_reported(&machine_id);
ensure!(!is_repotted,Error::<T>::IsReported);
        let machine_info =
            <online_profile::Pallet<T>>::machines_info(&machine_id).ok_or(Error::<T>::Unknown)?;
        // let machine_rented_gpu = <online_profile::Pallet<T>>::machine_rented_gpu(&machine_id);
        let gpu_num = machine_info.gpu_num();
        let gpu_free_count =  Self::free_mode_gpu_count_in_machine(&machine_id);
        ensure!(gpu_free_count >= rent_gpu_num ,Error::<T>::NoMoreGpu);

        // if gpu_num == 0 || duration == Zero::zero() {
        //     return Ok(().into())
        // }

        // 检查还有空闲的GPU
        // ensure!(rent_gpu_num + machine_rented_gpu <= gpu_num, Error::<T>::GPUNotEnough);

        // 租用必须是30min的整数倍
        // ensure!(duration % HALF_HOUR.into() == Zero::zero(), Error::<T>::OnlyHalfHourAllowed);

        // 检查machine_id状态是否可以租用
        // ensure!(
        //     machine_info.machine_status == MachineStatus::Online ||
        //         machine_info.machine_status == MachineStatus::Rented,
        //     Error::<T>::MachineNotRentable
        // );

        // 最大租用时间限制MaximumRentalDuration
        // let duration =
        //     duration.min((Self::maximum_rental_duration().saturating_mul(ONE_DAY)).into());

        // NOTE: 用户提交订单，每个gpu支付1000个dbc
        // <generic_func::Pallet<T>>::pay_fixed_tx_fee(renter.clone())
        //     .map_err(|_| Error::<T>::PayTxFeeFailed)?;

        // 获得machine_price(每天的价格)
        // 根据租用GPU数量计算价格
        // let machine_price =
        //     T::RTOps::get_machine_price(machine_info.calc_point(), rent_gpu_num, gpu_num)
        //         .ok_or(Error::<T>::GetMachinePriceFailed)?;

        // // 根据租用时长计算rent_fee
        // let rent_fee_value = machine_price
        //     .checked_mul(duration.saturated_into::<u64>())
        //     .ok_or(Error::<T>::Overflow)?
        //     .checked_div(ONE_DAY.into())
        //     .ok_or(Error::<T>::Overflow)?;
        // let rent_fee = <T as Config>::DbcPrice::get_dbc_amount_by_value(rent_fee_value)
        //     .ok_or(Error::<T>::Overflow)?;

        // // 获取用户租用的结束时间(块高)
        // let rent_end = duration.checked_add(&now).ok_or(Error::<T>::Overflow)?;

        let amount_stake =BalanceOf::<T>::from(gpu_free_count).checked_mul(&BalanceOf::<T>::from(AMOUNT_DEPOSIT_FREE_MODE)).ok_or(Error::<T>::Overflow)?;
        // 质押用户的资金，并修改机器状态
        Self::update_stake_amount_for_free_mode(&renter, amount_stake, false)
            .map_err(|_| Error::<T>::InsufficientValue)?;

        // let rent_id = Self::get_new_rent_id();

        // let mut machine_rent_order = Self::machine_rent_order(&machine_id);
        // let rentable_gpu_index = machine_rent_order.gen_rentable_gpu(rent_gpu_num, gpu_num);
        // ItemList::add_item(&mut machine_rent_order.rent_order, rent_id);

        // // 改变online_profile状态，影响机器佣金
        // T::RTOps::change_machine_status_on_rent_start(&machine_id, rent_gpu_num)
        //     .map_err(|_| Error::<T>::Unknown)?;

        // RentInfo::<T>::insert(
        //     &rent_id,
        //     RentOrderDetail::new(
        //         machine_id.clone(),
        //         renter.clone(),
        //         now,
        //         rent_end,
        //         rent_fee,
        //         rent_gpu_num,
        //         rentable_gpu_index,
        //     ),
        // );

        // UserOrder::<T>::mutate(&renter, |user_order| {
        //     ItemList::add_item(user_order, rent_id);
        // });

        // RentEnding::<T>::mutate(rent_end, |rent_ending| {
        //     ItemList::add_item(rent_ending, rent_id);
        // });

        // ConfirmingOrder::<T>::mutate(now + WAITING_CONFIRMING_DELAY.into(), |pending_confirming| {
        //     ItemList::add_item(pending_confirming, rent_id);
        // });

        // MachineRentOrder::<T>::insert(&machine_id, machine_rent_order);

        Self::deposit_event(Event::ExitFreeMode(
            renter,machine_id,rent_gpu_num
        ));
        Ok(().into())
    }



    fn report_base(
        renter: T::AccountId,
        machine_id: MachineId,

    ) -> DispatchResultWithPostInfo {

        let now = <frame_system::Pallet<T>>::block_number();
       let begin_reported =  Self::is_reported(&machine_id);
        ensure!(!begin_reported,Error::<T>::IsReported);
        IsReported::<T>::insert(&machine_id,true);
        let machine_info =
            <online_profile::Pallet<T>>::machines_info(&machine_id).ok_or(Error::<T>::Unknown)?;
        // let machine_rented_gpu = <online_profile::Pallet<T>>::machine_rented_gpu(&machine_id);
        // let gpu_num_total = machine_info.gpu_num();
        let gpu_num_free =  Self::free_mode_gpu_count_in_machine(&machine_id);
        ensure!(gpu_num_free > 0 ,Error::<T>::NotFreeMode);


       // let is_deposited = Self::is_report_user(&renter);

            Self::deposit_for_report(&renter, BalanceOf::<T>::from(AMOUNT_DEPOSIT_REPORT), true)
            .map_err(|_| Error::<T>::InsufficientValue)?;
            let idx = Self::reported_idx();
// let items = Self::report_items(&idx);

            ReportItems::<T>::insert(idx,ReportItem{
                // reporter:Some(renter.clone()),

                machine_id:machine_id.clone(),
                approve:0,
                reject:0,
                id:idx,
            });

            ReportedIdx::<T>::set(idx +1);

        // if gpu_num == 0 || duration == Zero::zero() {
        //     return Ok(().into())
        // }

        // 检查还有空闲的GPU
        // ensure!(rent_gpu_num + machine_rented_gpu <= gpu_num, Error::<T>::GPUNotEnough);

        // 租用必须是30min的整数倍
        // ensure!(duration % HALF_HOUR.into() == Zero::zero(), Error::<T>::OnlyHalfHourAllowed);

        // 检查machine_id状态是否可以租用
        // ensure!(
        //     machine_info.machine_status == MachineStatus::Online ||
        //         machine_info.machine_status == MachineStatus::Rented,
        //     Error::<T>::MachineNotRentable
        // );

        // 最大租用时间限制MaximumRentalDuration
        // let duration =
        //     duration.min((Self::maximum_rental_duration().saturating_mul(ONE_DAY)).into());

        // NOTE: 用户提交订单，每个gpu支付1000个dbc
        // <generic_func::Pallet<T>>::pay_fixed_tx_fee(renter.clone())
        //     .map_err(|_| Error::<T>::PayTxFeeFailed)?;

        // 获得machine_price(每天的价格)
        // 根据租用GPU数量计算价格
        // let machine_price =
        //     T::RTOps::get_machine_price(machine_info.calc_point(), rent_gpu_num, gpu_num)
        //         .ok_or(Error::<T>::GetMachinePriceFailed)?;

        // // 根据租用时长计算rent_fee
        // let rent_fee_value = machine_price
        //     .checked_mul(duration.saturated_into::<u64>())
        //     .ok_or(Error::<T>::Overflow)?
        //     .checked_div(ONE_DAY.into())
        //     .ok_or(Error::<T>::Overflow)?;
        // let rent_fee = <T as Config>::DbcPrice::get_dbc_amount_by_value(rent_fee_value)
        //     .ok_or(Error::<T>::Overflow)?;

        // // 获取用户租用的结束时间(块高)
        // let rent_end = duration.checked_add(&now).ok_or(Error::<T>::Overflow)?;

        // let amount_stake =BalanceOf::<T>::from(gpu_free_count).checked_mul(&BalanceOf::<T>::from(AMOUNT_DEPOSIT_FREE_MODE)).ok_or(Error::<T>::Overflow)?;
        // 质押用户的资金，并修改机器状态
        // Self::update_stake_amount_for_free_mode(&renter, amount_stake, false)
        //     .map_err(|_| Error::<T>::InsufficientValue)?;

        // let rent_id = Self::get_new_rent_id();

        // let mut machine_rent_order = Self::machine_rent_order(&machine_id);
        // let rentable_gpu_index = machine_rent_order.gen_rentable_gpu(rent_gpu_num, gpu_num);
        // ItemList::add_item(&mut machine_rent_order.rent_order, rent_id);

        // // 改变online_profile状态，影响机器佣金
        // T::RTOps::change_machine_status_on_rent_start(&machine_id, rent_gpu_num)
        //     .map_err(|_| Error::<T>::Unknown)?;

        // RentInfo::<T>::insert(
        //     &rent_id,
        //     RentOrderDetail::new(
        //         machine_id.clone(),
        //         renter.clone(),
        //         now,
        //         rent_end,
        //         rent_fee,
        //         rent_gpu_num,
        //         rentable_gpu_index,
        //     ),
        // );

        // UserOrder::<T>::mutate(&renter, |user_order| {
        //     ItemList::add_item(user_order, rent_id);
        // });

        // RentEnding::<T>::mutate(rent_end, |rent_ending| {
        //     ItemList::add_item(rent_ending, rent_id);
        // });

        // ConfirmingOrder::<T>::mutate(now + WAITING_CONFIRMING_DELAY.into(), |pending_confirming| {
        //     ItemList::add_item(pending_confirming, rent_id);
        // });

        // MachineRentOrder::<T>::insert(&machine_id, machine_rent_order);

        Self::deposit_event(Event::ReportFreeMode(
            renter,machine_id,idx
        ));
        Ok(().into())
    }




    fn update_notary_addr(
        renter: T::AccountId,
        value :bool

    ) -> DispatchResultWithPostInfo {

    
        NotaryAddr::<T>::insert(&renter,value);
    
        Self::deposit_event(Event::UpdateNotaryAddr(
            renter,value
        ));
        Ok(().into())
    }




    fn verify_report(
        renter: T::AccountId,
        id :u32,
        is_approve:bool

    ) -> DispatchResultWithPostInfo {

        let item =         Self::report_items(id);
        let verified_count = item.approve + item.reject;
        ensure!(verified_count <3,Error::<T>::Verified);
let is_verified =         Self::is_report_verified(id,&renter);
ensure!(!is_verified,Error::<T>::Verified);
IsReportVerified::<T>::insert(id,&renter,true);

       if is_approve {
            ReportItems::<T>::insert(id,ReportItem{
                // reporter:Some(item.reporter),
                machine_id:item.machine_id,
                approve:item.approve + 1,
                reject:item.reject,
                id:item.id,
            });
       }else {
        ReportItems::<T>::insert(id,ReportItem{
            // reporter:Some(item.reporter),
            machine_id:item.machine_id,
            approve:item.approve ,
            reject:item.reject + 1,
            id:item.id,
        });
       }

       if verified_count ==2 {
        // todo 验证人数量达到3个,给出最后的结果,至此举报事件处理完成 
       }
       
     
        Ok(().into())
    }


    //
    // fn relet_machine_by_block(
    //     renter: T::AccountId,
    //     rent_id: RentOrderId,
    //     duration: T::BlockNumber,
    // ) -> DispatchResultWithPostInfo {
    //     let mut rent_info = Self::rent_info(&rent_id).ok_or(Error::<T>::Unknown)?;
    //     let old_rent_end = rent_info.rent_end;
    //     let machine_id = rent_info.machine_id.clone();
    //     let gpu_num = rent_info.gpu_num;
    //
    //     // 续租允许10分钟及以上
    //     ensure!(duration >= (10 * ONE_MINUTE).into(), Error::<T>::ReletTooShort);
    //     ensure!(rent_info.renter == renter, Error::<T>::NotMachineRenter);
    //     ensure!(rent_info.rent_status == RentStatus::Renting, Error::<T>::NoOrderExist);
    //
    //     let machine_info =
    //         <online_profile::Pallet<T>>::machines_info(&machine_id).ok_or(Error::<T>::Unknown)?;
    //     let calc_point = machine_info.calc_point();
    //
    //     // 确保租用时间不超过设定的限制，计算最多续费租用到
    //     let now = <frame_system::Pallet<T>>::block_number();
    //     // 最大结束块高为 今天租用开始的时间 + 60天
    //     // 60 days * 24 hour/day * 60 min/hour * 2 block/min
    //     let max_rent_end = now.checked_add(&(60 * ONE_DAY).into()).ok_or(Error::<T>::Overflow)?;
    //     let wanted_rent_end = old_rent_end + duration;
    //
    //     // 计算实际可续租时间 (块高)
    //     let add_duration: T::BlockNumber = if max_rent_end >= wanted_rent_end {
    //         duration
    //     } else {
    //         max_rent_end.saturating_sub(old_rent_end)
    //     };
    //
    //     if add_duration == 0u32.into() {
    //         return Ok(().into())
    //     }
    //
    //     // 计算rent_fee
    //     let machine_price =
    //         T::RTOps::get_machine_price(calc_point, gpu_num, machine_info.gpu_num())
    //             .ok_or(Error::<T>::GetMachinePriceFailed)?;
    //     let rent_fee_value = machine_price
    //         .checked_mul(add_duration.saturated_into::<u64>())
    //         .ok_or(Error::<T>::Overflow)?
    //         .checked_div(ONE_DAY.into())
    //         .ok_or(Error::<T>::Overflow)?;
    //     let rent_fee = <T as Config>::DbcPrice::get_dbc_amount_by_value(rent_fee_value)
    //         .ok_or(Error::<T>::Overflow)?;
    //
    //     // 检查用户是否有足够的资金，来租用机器
    //     let user_balance = <T as Config>::Currency::free_balance(&renter);
    //     ensure!(rent_fee < user_balance, Error::<T>::InsufficientValue);
    //
    //     Self::pay_rent_fee(&renter, machine_id.clone(), machine_info.machine_stash, rent_fee)?;
    //
    //     // 获取用户租用的结束时间
    //     rent_info.rent_end =
    //         rent_info.rent_end.checked_add(&add_duration).ok_or(Error::<T>::Overflow)?;
    //
    //     RentEnding::<T>::mutate(old_rent_end, |old_rent_ending| {
    //         ItemList::rm_item(old_rent_ending, &rent_id);
    //     });
    //     RentEnding::<T>::mutate(rent_info.rent_end, |rent_ending| {
    //         ItemList::add_item(rent_ending, rent_id);
    //     });
    //
    //     MachineRenterRentedOrders::<T>::mutate(&machine_id, &renter, |details| {
    //         details.push(MachineRenterRentedOrderDetail {
    //             rent_start: rent_info.rent_start,
    //             rent_end: rent_info.rent_end,
    //             rent_id: rent_id.clone(),
    //         });
    //     });
    //
    //     RentInfo::<T>::insert(&rent_id, rent_info);
    //
    //     Self::deposit_event(Event::Relet(
    //         rent_id,
    //         renter,
    //         machine_id,
    //         gpu_num,
    //         add_duration,
    //         rent_fee,
    //     ));
    //     Ok(().into())
    // }

    // // 获取一个新的租用订单的ID
    // pub fn get_new_rent_id() -> RentOrderId {
    //     let rent_id = Self::next_rent_id();
    //
    //     let new_rent_id = loop {
    //         let new_rent_id = if rent_id == u64::MAX { 0 } else { rent_id + 1 };
    //         if !RentInfo::<T>::contains_key(new_rent_id) {
    //             break new_rent_id
    //         }
    //     };
    //
    //     NextRentId::<T>::put(new_rent_id);
    //
    //     rent_id
    // }
    //
    // // NOTE: 银河竞赛开启前，租金付给stash账户；开启后租金转到销毁账户
    // // NOTE: 租金付给stash账户时，检查是否满足单卡10w/$300的质押条件，不满足，先质押.
    // fn pay_rent_fee(
    //     renter: &T::AccountId,
    //     machine_id: MachineId,
    //     machine_stash: T::AccountId,
    //     fee_amount: BalanceOf<T>,
    // ) -> DispatchResult {
    //     let rent_fee_pot = Self::rent_fee_pot().ok_or(Error::<T>::UndefinedRentPot)?;
    //
    //     let destroy_percent = <online_profile::Pallet<T>>::rent_fee_destroy_percent();
    //
    //     let fee_to_destroy = destroy_percent * fee_amount;
    //     let fee_to_stash = fee_amount.checked_sub(&fee_to_destroy).ok_or(Error::<T>::Overflow)?;
    //
    //     <T as pallet::Config>::Currency::transfer(renter, &machine_stash, fee_to_stash, KeepAlive)?;
    //     <T as pallet::Config>::Currency::transfer(
    //         renter,
    //         &rent_fee_pot,
    //         fee_to_destroy,
    //         KeepAlive,
    //     )?;
    //     let _ = T::RTOps::change_machine_rent_fee(
    //         machine_stash,
    //         machine_id,
    //         fee_to_destroy,
    //         fee_to_stash,
    //     );
    //     Ok(())
    // }

    // // 定时检查机器是否30分钟没有上线
    // fn check_machine_starting_status() -> Result<(), ()> {
    //     let now = <frame_system::Pallet<T>>::block_number();
    //
    //     if !<ConfirmingOrder<T>>::contains_key(now) {
    //         return Ok(())
    //     }
    //
    //     let pending_confirming = Self::confirming_order(now);
    //     for rent_id in pending_confirming {
    //         let rent_info = Self::rent_info(&rent_id).ok_or(())?;
    //
    //         Self::clean_order(&rent_info.renter, rent_id)?;
    //         T::RTOps::change_machine_status_on_confirm_expired(
    //             &rent_info.machine_id,
    //             rent_info.gpu_num,
    //         )?;
    //     }
    //     Ok(())
    // }

    // // -Write: MachineRentOrder, RentEnding, RentOrder,
    // // UserOrder, ConfirmingOrder
    // fn clean_order(who: &T::AccountId, rent_order_id: RentOrderId) -> Result<(), ()> {
    //     let mut user_order = Self::user_order(who);
    //     ItemList::rm_item(&mut user_order, &rent_order_id);
    //
    //     let rent_info = Self::rent_info(rent_order_id).ok_or(())?;
    //
    //     // return back staked money!
    //     if !rent_info.stake_amount.is_zero() {
    //         let _ = Self::change_renter_total_stake(who, rent_info.stake_amount, false);
    //     }
    //
    //     let mut rent_ending = Self::rent_ending(rent_info.rent_end);
    //     ItemList::rm_item(&mut rent_ending, &rent_order_id);
    //
    //     let pending_confirming_deadline = rent_info.rent_start + WAITING_CONFIRMING_DELAY.into();
    //     let mut pending_confirming = Self::confirming_order(&pending_confirming_deadline);
    //     ItemList::rm_item(&mut pending_confirming, &rent_order_id);
    //
    //     let mut machine_rent_order = Self::machine_rent_order(&rent_info.machine_id);
    //     machine_rent_order.clean_expired_order(rent_order_id, rent_info.gpu_index);
    //
    //     MachineRentOrder::<T>::insert(&rent_info.machine_id, machine_rent_order);
    //     if rent_ending.is_empty() {
    //         RentEnding::<T>::remove(rent_info.rent_end);
    //     } else {
    //         RentEnding::<T>::insert(rent_info.rent_end, rent_ending);
    //     }
    //     RentInfo::<T>::remove(rent_order_id);
    //     if user_order.is_empty() {
    //         UserOrder::<T>::remove(who);
    //     } else {
    //         UserOrder::<T>::insert(who, user_order);
    //     }
    //     if pending_confirming.is_empty() {
    //         ConfirmingOrder::<T>::remove(pending_confirming_deadline);
    //     } else {
    //         ConfirmingOrder::<T>::insert(pending_confirming_deadline, pending_confirming);
    //     }
    //     Ok(())
    // }

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

    fn update_stake_amount_for_free_mode(
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



    fn deposit_for_report(
        who: &T::AccountId,
        amount: BalanceOf<T>,
        is_add: bool,
    ) -> Result<(), ()> {
        let current_stake = Self::amount_staked_for_report(who);

        let new_stake = if is_add {
            ensure!(<T as Config>::Currency::can_reserve(who, amount), ());
            <T as Config>::Currency::reserve(who, amount).map_err(|_| ())?;
            current_stake.checked_add(&amount).ok_or(())?
        } else {
            ensure!(current_stake >= amount, ());
            let _ = <T as Config>::Currency::unreserve(who, amount);
            current_stake.checked_sub(&amount).ok_or(())?
        };
        AmountStakedForReport::<T>::insert(who, new_stake);
        Ok(())
    }

    // // 这里修rentMachine模块通知onlineProfile机器已经租用完成，
    // // onlineProfile判断机器是否需要变成online状态，或者记录下之前是租用状态，
    // // 以便机器再次上线时进行正确的惩罚
    // fn check_if_rent_finished() -> Result<(), ()> {
    //     let now = <frame_system::Pallet<T>>::block_number();
    //     if !<RentEnding<T>>::contains_key(now) {
    //         return Ok(())
    //     }
    //     let pending_ending = Self::rent_ending(now);
    //
    //     for rent_id in pending_ending {
    //         let rent_info = Self::rent_info(&rent_id).ok_or(())?;
    //         let machine_id = rent_info.machine_id.clone();
    //         let rent_duration = now.saturating_sub(rent_info.rent_start);
    //
    //         // NOTE: 只要机器还有租用订单(租用订单>1)，就不修改成online状态。
    //         let is_last_rent = Self::is_last_rent(&machine_id, &rent_info.renter)?;
    //         let _ = T::RTOps::change_machine_status_on_rent_end(
    //             &machine_id,
    //             rent_info.gpu_num,
    //             rent_duration,
    //             is_last_rent.0,
    //             is_last_rent.1,
    //             rent_info.renter.clone(),
    //         );
    //
    //         let _ = Self::clean_order(&rent_info.renter, rent_id);
    //     }
    //     Ok(())
    // }

    // // 当没有正在租用的机器时，可以修改得分快照
    // // 判断machine_id的订单是否只有1个
    // // 判断renter是否只租用了machine_id一次
    // fn is_last_rent(machine_id: &MachineId, renter: &T::AccountId) -> Result<(bool, bool), ()> {
    //     let machine_order = Self::machine_rent_order(machine_id);
    //     let mut machine_order_count = 0;
    //     let mut renter_order_count = 0;
    //
    //     // NOTE: 一定是正在租用的机器才算，正在确认中的租用不算
    //     for order_id in machine_order.rent_order {
    //         let rent_info = Self::rent_info(order_id).ok_or(())?;
    //         if renter == &rent_info.renter {
    //             renter_order_count = renter_order_count.saturating_add(1);
    //         }
    //         if matches!(rent_info.rent_status, RentStatus::Renting) {
    //             machine_order_count = machine_order_count.saturating_add(1);
    //         }
    //     }
    //     Ok((machine_order_count < 2, renter_order_count < 2))
    // }
    //
    // pub fn get_rent_ids(machine_id: MachineId, renter: &T::AccountId) -> Vec<RentOrderId> {
    //     let machine_orders = Self::machine_rent_order(machine_id);
    //
    //     let mut rent_ids: Vec<RentOrderId> = Vec::new();
    //     for order_id in machine_orders.rent_order {
    //         if let Some(rent_info) = Self::rent_info(order_id) {
    //             if renter == &rent_info.renter && rent_info.rent_status == RentStatus::Renting {
    //                 rent_ids.push(order_id);
    //             }
    //         }
    //     }
    //     rent_ids
    // }
    //
    // pub fn get_rent_id_of_renting_dbc_machine_by_owner(
    //     machine_id: &MachineId,
    // ) -> Option<RentOrderId> {
    //     let machine_order = Self::machine_rent_order(machine_id.clone());
    //     if let Some(machine_info) = online_profile::Pallet::<T>::machines_info(machine_id) {
    //         if machine_order.rent_order.len() == 1 {
    //             let rent_id = machine_order.rent_order[0];
    //             if let Some(rent_info) = Self::rent_info(machine_order.rent_order[0]) {
    //                 if rent_info.rent_status == RentStatus::Renting &&
    //                     rent_info.renter == machine_info.controller
    //                 {
    //                     return Some(rent_id)
    //                 }
    //             }
    //         }
    //     };
    //     None
    // }
}
//
// impl<T: Config> MachineInfoTrait for Pallet<T> {
//     type BlockNumber = T::BlockNumber;
//     //
//     // fn get_machine_calc_point(machine_id: MachineId) -> u64 {
//     //     let machine_info_result = online_profile::Pallet::<T>::machines_info(machine_id);
//     //     if let Some(machine_info) = machine_info_result {
//     //         return machine_info.calc_point()
//     //     }
//     //     0
//     // }
//
//     fn get_machine_cpu_rate(machine_id: MachineId) -> u64 {
//         let machine_info_result = online_profile::Pallet::<T>::machines_info(machine_id);
//         if let Some(machine_info) = machine_info_result {
//             return machine_info.cpu_rate()
//         }
//         0
//     }
//
//     fn get_machine_gpu_num(machine_id: MachineId) -> u64 {
//         let machine_info_result = online_profile::Pallet::<T>::machines_info(machine_id);
//         if let Some(machine_info) = machine_info_result {
//             return machine_info.gpu_num() as u64
//         }
//         0
//     }
//
//     // get machine rent end block number by owner
//     fn get_rent_end_at(
//         machine_id: MachineId,
//         rent_id: RentOrderId,
//     ) -> Result<T::BlockNumber, &'static str> {
//         let machine_info = online_profile::Pallet::<T>::machines_info(&machine_id)
//             .ok_or(Error::<T>::MachineNotFound.as_str())?;
//
//         let renter_controller = machine_info.controller;
//         let renter_stash = machine_info.machine_stash;
//
//         let rent_info = Self::rent_info(rent_id).ok_or(Error::<T>::MachineNotRented.as_str())?;
//
//         if rent_info.machine_id != machine_id {
//             return Err(Error::<T>::NotMachineRenter.as_str())
//         }
//
//         if rent_info.renter != renter_controller && rent_info.renter != renter_stash {
//             return Err(Error::<T>::NotMachineRenter.as_str())
//         }
//
//         Ok(rent_info.rent_end)
//     }
//
//     // fn is_machine_owner(machine_id: MachineId, evm_address: H160) -> Result<bool, &'static str> {
//     //     let account = Self::evm_address_to_account(evm_address)
//     //         .ok_or(Error::<T>::NotMachineOwner.as_str())?;
//     //
//     //     let machine_info = online_profile::Pallet::<T>::machines_info(machine_id)
//     //         .ok_or(Error::<T>::MachineNotFound.as_str())?;
//     //
//     //     return Ok(machine_info.controller == account || machine_info.machine_stash == account)
//     // }
//     //
//     // fn get_usdt_machine_rent_fee(
//     //     machine_id: MachineId,
//     //     duration: T::BlockNumber,
//     //     rent_gpu_num: u32,
//     // ) -> Result<u64, &'static str> {
//     //     let machine_info = <online_profile::Pallet<T>>::machines_info(&machine_id)
//     //         .ok_or(Error::<T>::Unknown.as_str())?;
//     //
//     //     let machine_price = T::RTOps::get_machine_price(
//     //         machine_info.calc_point(),
//     //         rent_gpu_num,
//     //         machine_info.gpu_num(),
//     //     )
//     //     .ok_or(Error::<T>::GetMachinePriceFailed)?;
//     //
//     //     // 根据租用时长计算rent_fee
//     //     let rent_fee_value = machine_price
//     //         .checked_mul(duration.saturated_into::<u64>())
//     //         .ok_or(Error::<T>::Overflow)?
//     //         .checked_div(ONE_DAY.into())
//     //         .ok_or(Error::<T>::Overflow)?;
//     //     Ok(rent_fee_value.saturated_into::<u64>())
//     // }
//     //
//     // fn get_dlc_machine_rent_fee(
//     //     machine_id: MachineId,
//     //     duration: T::BlockNumber,
//     //     rent_gpu_num: u32,
//     // ) -> Result<u64, &'static str> {
//     //     let machine_info = <online_profile::Pallet<T>>::machines_info(&machine_id)
//     //         .ok_or(Error::<T>::Unknown.as_str())?;
//     //
//     //     let machine_price = T::RTOps::get_machine_price(
//     //         machine_info.calc_point(),
//     //         rent_gpu_num,
//     //         machine_info.gpu_num(),
//     //     )
//     //     .ok_or(Error::<T>::GetMachinePriceFailed)?;
//     //
//     //     // 根据租用时长计算rent_fee
//     //     let rent_fee_value = machine_price
//     //         .checked_mul(duration.saturated_into::<u64>())
//     //         .ok_or(Error::<T>::Overflow)?
//     //         .checked_div(ONE_DAY.into())
//     //         .ok_or(Error::<T>::Overflow)?;
//     //     let rent_fee_value =
//     //         Perbill::from_rational(25u32, 100u32) * rent_fee_value + rent_fee_value;
//     //
//     //     let rent_fee = <T as Config>::DbcPrice::get_dlc_amount_by_value(rent_fee_value)
//     //         .ok_or(Error::<T>::Overflow)?;
//     //     Ok(rent_fee.saturated_into::<u64>())
//     // }
//     //
//     // fn get_dbc_machine_rent_fee(
//     //     machine_id: MachineId,
//     //     duration: T::BlockNumber,
//     //     rent_gpu_num: u32,
//     // ) -> Result<u64, &'static str> {
//     //     let machine_info = <online_profile::Pallet<T>>::machines_info(&machine_id)
//     //         .ok_or(Error::<T>::Unknown.as_str())?;
//     //
//     //     let machine_price = T::RTOps::get_machine_price(
//     //         machine_info.calc_point(),
//     //         rent_gpu_num,
//     //         machine_info.gpu_num(),
//     //     )
//     //     .ok_or(Error::<T>::GetMachinePriceFailed)?;
//     //
//     //     // 根据租用时长计算rent_fee
//     //     let rent_fee_value = machine_price
//     //         .checked_mul(duration.saturated_into::<u64>())
//     //         .ok_or(Error::<T>::Overflow)?
//     //         .checked_div(ONE_DAY.into())
//     //         .ok_or(Error::<T>::Overflow)?;
//     //     let rent_fee = <T as Config>::DbcPrice::get_dbc_amount_by_value(rent_fee_value)
//     //         .ok_or(Error::<T>::Overflow)?;
//     //     Ok(rent_fee.saturated_into::<u64>())
//     // }
// }
