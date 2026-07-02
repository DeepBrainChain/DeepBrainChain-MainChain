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
    traits::{
        Currency,
        ExistenceRequirement::{AllowDeath, KeepAlive},
        ReservableCurrency,
    },
    PalletId,
};
use frame_system::{ensure_root, ensure_signed, pallet_prelude::*};
use sp_core::H160;
use sp_runtime::{
    traits::{AccountIdConversion, CheckedAdd, CheckedSub, SaturatedConversion, Saturating, Zero},
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
        /// [Thread B ③ · 托管] 租金托管账户的 PalletId 来源。托管账户 = into_account，pallet 私有、
        /// 只由 settle_escrow 支付。租用时租金押入此账户，退租/离线按时长结算。
        #[pallet::constant]
        type RentEscrowPalletId: Get<PalletId>;
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

    /// [Thread B ③ · 托管] 每个 rent_id 当前托管在 escrow 账户里的租金总额。
    /// **"存在与否"即迁移边界**：升级后 confirm_rent 的新单会写入此项；升级前的旧单（已按老规则即时付给矿工）
    /// 无此项 → settle_escrow 对其 no-op。零 clawback、零批量迁移。
    #[pallet::storage]
    #[pallet::getter(fn escrowed_fee)]
    pub(super) type EscrowedFee<T: Config> =
        StorageMap<_, Blake2_128Concat, RentOrderId, BalanceOf<T>, ValueQuery>;

    /// [Thread B ③ · 托管] 结算时冻结的销毁比例快照（confirm 时读 online_profile::rent_fee_destroy_percent 存入），
    /// 防治理中途改比例回溯影响已托管订单。Absent → 结算时回退读当前值（旧单/未快照单）。
    #[pallet::storage]
    #[pallet::getter(fn rent_escrow_destroy_percent)]
    pub(super) type RentEscrowDestroyPercent<T: Config> =
        StorageMap<_, Blake2_128Concat, RentOrderId, Perbill>;

    /// [Thread B ③ · 托管] 直转失败(受款方被冻结/拉黑)时暂存的退款/付款，受款方自行 claim_dbc_payout 领取，
    /// 保证 settle_escrow 永不因单个受款方问题 revert 卡死（对标 RentDBC pendingDbcPayout）。
    #[pallet::storage]
    #[pallet::getter(fn pending_dbc_payout)]
    pub(super) type PendingDbcPayout<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

    /// [Thread B ③ · 托管] pending 退款总额（不变量校验用：Σescrow + Σpending 应与托管账户余额一致）。
    #[pallet::storage]
    #[pallet::getter(fn total_pending_dbc_payout)]
    pub(super) type TotalPendingDbcPayout<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    /// spec 410: 订单创建时快照矿工的 receiver，防止 bait-and-switch。
    /// Absent 等价于「用 stash 自己收」（向后兼容：升级前的旧订单无此快照）。
    #[pallet::storage]
    #[pallet::getter(fn rent_order_receiver)]
    pub(super) type RentOrderReceiver<T: Config> =
        StorageMap<_, Blake2_128Concat, RentOrderId, T::AccountId>;

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

    // The current storage version.
    #[pallet::storage]
    #[pallet::getter(fn storage_version)]
    pub(super) type StorageVersion<T: Config> = StorageValue<_, u16, ValueQuery>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // 设置机器租金支付目标地址
        #[pallet::call_index(0)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(20_000_000, 0).saturating_add(<T as frame_system::Config>::DbWeight::get().reads_writes(5, 4)))]
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
        #[pallet::weight(frame_support::weights::Weight::from_parts(50_000_000, 0).saturating_add(<T as frame_system::Config>::DbWeight::get().reads_writes(25, 20)))]
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
        #[pallet::weight(frame_support::weights::Weight::from_parts(50_000_000, 0).saturating_add(<T as frame_system::Config>::DbWeight::get().reads_writes(25, 20)))]
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

            // [Thread B ③ · 托管] 解押租客预付租金，改为**押入托管账户**（不再即时付给矿工）。
            //   退租/离线时由 settle_escrow 按已用时长结算：已用 burn+给矿工、未用退租客、离线罚≤24h。
            //   confirm 时快照当前销毁比例，防治理中途改比例回溯影响本单。
            Self::change_renter_total_stake(&renter, rent_info.stake_amount, false)
                .map_err(|_| Error::<T>::UnlockToPayFeeFailed)?;
            <T as pallet::Config>::Currency::transfer(
                &renter,
                &Self::escrow_account(),
                rent_info.stake_amount,
                KeepAlive,
            )?;
            EscrowedFee::<T>::insert(rent_id, rent_info.stake_amount);
            RentEscrowDestroyPercent::<T>::insert(
                rent_id,
                <online_profile::Pallet<T>>::rent_fee_destroy_percent(),
            );

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
        #[pallet::weight(frame_support::weights::Weight::from_parts(50_000_000, 0).saturating_add(<T as frame_system::Config>::DbWeight::get().reads_writes(25, 20)))]
        pub fn relet_machine(
            origin: OriginFor<T>,
            rent_id: RentOrderId,
            relet_duration: T::BlockNumber,
        ) -> DispatchResultWithPostInfo {
            let renter = ensure_signed(origin)?;
            Self::relet_machine_by_block(renter, rent_id, relet_duration)
        }

        #[pallet::call_index(4)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(20_000_000, 0).saturating_add(<T as frame_system::Config>::DbWeight::get().reads_writes(5, 4)))]
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

        /// [Thread B ③ · 托管] 租客主动提前退租。按已用时长结算：已用给矿工(减 burn)、未用退租客、
        /// **不罚**(penalty=0，矿工无过错)。质押 bond 不动。仅本人、仅 Renting 状态可调。
        #[pallet::call_index(5)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(30_000_000, 0).saturating_add(<T as frame_system::Config>::DbWeight::get().reads_writes(15, 12)))]
        pub fn end_rent(origin: OriginFor<T>, rent_id: RentOrderId) -> DispatchResultWithPostInfo {
            let renter = ensure_signed(origin)?;
            let now = <frame_system::Pallet<T>>::block_number();
            let rent_info = Self::rent_info(&rent_id).ok_or(Error::<T>::NoOrderExist)?;
            ensure!(rent_info.renter == renter, Error::<T>::NoOrderExist);
            ensure!(rent_info.rent_status == RentStatus::Renting, Error::<T>::NoOrderExist);
            // penalty=0（offline=false），结算时点=now
            Self::settle_and_finalize_rent(rent_id, &rent_info, now, false)
                .map_err(|_| Error::<T>::Unknown)?;
            Self::deposit_event(Event::RentEndedByUser(rent_id, renter));
            Ok(().into())
        }

        /// [Thread B ③ · 托管] 领取因结算直转失败(受款方曾被冻结/低于ED)而暂存的托管退款/付款。
        /// 任何账户领自己名下的 PendingDbcPayout；从托管账户转出。
        #[pallet::call_index(6)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(20_000_000, 0).saturating_add(<T as frame_system::Config>::DbWeight::get().reads_writes(3, 3)))]
        pub fn claim_dbc_payout(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let amount = PendingDbcPayout::<T>::get(&who);
            ensure!(!amount.is_zero(), Error::<T>::NothingToClaim);
            PendingDbcPayout::<T>::remove(&who);
            TotalPendingDbcPayout::<T>::mutate(|t| *t = t.saturating_sub(amount));
            // AllowDeath 同 pay_from_escrow_or_defer：托管是过路账户，领走最后一笔可清零；
            //   若这里用 KeepAlive，当 pending 恰为托管仅剩余额时会拒付 → 资金被永久困住。
            <T as pallet::Config>::Currency::transfer(
                &Self::escrow_account(),
                &who,
                amount,
                AllowDeath,
            )?;
            Self::deposit_event(Event::DbcPayoutClaimed(who, amount));
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
        // S2: 收租钱包转账失败，已回退到 stash。(machine_stash, intended_receiver, amount)
        RentReceiverPayoutFallback(T::AccountId, T::AccountId, BalanceOf<T>),
        // 租金记账/补质押失败（资金已转移，仅记账环节出错），仅用于可观测，不回滚。(rent_id)
        RentFeeAccountingFailed(RentOrderId),
        // [Thread B ③] 托管结算完成 (rent_id, used_fee 已用给矿工基数, renter_refund, penalty)
        RentEscrowSettled(RentOrderId, BalanceOf<T>, BalanceOf<T>, BalanceOf<T>),
        // [Thread B ③] 托管付款直转失败已转 pending，受款方自行 claim (recipient, amount)
        DbcPayoutDeferred(T::AccountId, BalanceOf<T>),
        // [Thread B ③] 受款方领取暂存的托管退款 (recipient, amount)
        DbcPayoutClaimed(T::AccountId, BalanceOf<T>),
        // [Thread B ③] 租客主动提前退租 (rent_id, renter)
        RentEndedByUser(RentOrderId, T::AccountId),
        // [Thread B ③] 机器离线触发的租约提前结算 (rent_id, machine_id)
        RentSettledOnOffline(RentOrderId, MachineId),
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
        /// 请求时段不在机器允许出租的时段内，或时长不足 2 小时
        OutOfRentalSchedule,
        /// [Thread B ③] claim_dbc_payout：该账户名下无暂存的托管退款
        NothingToClaim,
        InvalidRentGpuNum,
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

        // [审计修 H3/round2] 拒绝 0 卡租用：rent_gpu_num==0 会以 ~0 租金创建空订单，可零成本在同一 rent_end 堆
        //   大量订单放大 on_finalize 强制结算量。要求 ≥1 卡，使订单数受真实租金经济约束（每机订单再受 gpu_num 上限约束）。
        ensure!(rent_gpu_num > 0, Error::<T>::InvalidRentGpuNum);

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

        // 分时段出租校验（TimeSlot 模式下必须落在允许时段内，且 ≥ 2 小时）
        // DBC 主网块时间 6 秒 = 6000ms（与 runtime MILLISECS_PER_BLOCK 一致）
        const MILLISECS_PER_BLOCK: u64 = 6_000;
        let start_ts_ms = <online_profile::Pallet<T>>::current_time_ms();
        let duration_ms: u64 = duration.saturated_into::<u64>()
            .checked_mul(MILLISECS_PER_BLOCK).ok_or(Error::<T>::Overflow)?;
        let end_ts_ms = start_ts_ms.checked_add(duration_ms).ok_or(Error::<T>::Overflow)?;
        ensure!(
            <online_profile::Pallet<T>>::is_rental_schedule_allowed(
                &machine_id, start_ts_ms, end_ts_ms
            ),
            Error::<T>::OutOfRentalSchedule
        );

        // NOTE: 用户提交订单，需要扣除10个DBC
        <generic_func::Pallet<T>>::pay_fixed_tx_fee(renter.clone())
            .map_err(|_| Error::<T>::PayTxFeeFailed)?;

        // 获得machine_price(每天的价格) = 系统自动定价 + 卡主额外加价
        // 根据租用GPU数量计算价格
        let system_price =
            T::RTOps::get_machine_price(machine_info.calc_point(), rent_gpu_num, gpu_num)
                .ok_or(Error::<T>::GetMachinePriceFailed)?;
        let extra_price_per_gpu = <online_profile::Pallet<T>>::machine_extra_price(&machine_id);
        let extra_price = extra_price_per_gpu
            .checked_mul(rent_gpu_num as u64)
            .ok_or(Error::<T>::Overflow)?;
        let machine_price = system_price.checked_add(extra_price).ok_or(Error::<T>::Overflow)?;

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

        // spec 410: 快照矿工当前的独立收租钱包，防止 confirm 窗口内 bait-and-switch
        if let Some(snapshot) =
            <online_profile::Pallet<T>>::stash_rent_receiver(&machine_info.machine_stash)
        {
            RentOrderReceiver::<T>::insert(&rent_id, snapshot);
        }

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
        let extra_price = <online_profile::Pallet<T>>::machine_extra_price(&machine_id)
            .checked_mul(gpu_num as u64).ok_or(Error::<T>::Overflow)?;
        let machine_price = system_price.checked_add(extra_price).ok_or(Error::<T>::Overflow)?;
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

        // [Thread B ③ · 托管] 续租费：已托管订单 → 加进托管（延长的 rent_end 会被 settle 一并按比例结算）；
        //   旧单(升级前、未托管) → 保持老规则即时付，避免混账。
        if EscrowedFee::<T>::contains_key(rent_id) {
            <T as Config>::Currency::transfer(
                &renter,
                &Self::escrow_account(),
                rent_fee,
                KeepAlive,
            )?;
            EscrowedFee::<T>::mutate(rent_id, |f| *f = f.saturating_add(rent_fee));
        } else {
            Self::pay_rent_fee(
                &renter,
                machine_id.clone(),
                machine_info.machine_stash,
                rent_id,
                rent_fee,
            )?;
        }

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

    /// [Thread B ③ · 托管] 租金托管账户（pallet 私有；只由 settle_escrow 支付）。
    pub fn escrow_account() -> T::AccountId {
        T::RentEscrowPalletId::get().into_account_truncating()
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
        rent_id: RentOrderId,
        fee_amount: BalanceOf<T>,
    ) -> DispatchResult {
        // 未配置销毁池时优雅回退（对齐 terminating-rental）：不销毁，全额计给卡主，
        // 避免缺少 RentFeePot 配置时 confirm_rent / relet 直接失败。
        let maybe_pot = Self::rent_fee_pot();

        let destroy_percent = <online_profile::Pallet<T>>::rent_fee_destroy_percent();

        let fee_to_destroy =
            if maybe_pot.is_some() { destroy_percent * fee_amount } else { Zero::zero() };
        let fee_to_stash = fee_amount.checked_sub(&fee_to_destroy).ok_or(Error::<T>::Overflow)?;

        // spec 410: 优先读订单创建时的快照（防 bait-and-switch）；升级前旧订单无快照，
        // 则回退到矿工当前设置；仍未设置则回退到 stash
        let rent_receiver = Self::rent_order_receiver(&rent_id)
            .unwrap_or_else(|| <online_profile::Pallet<T>>::effective_rent_receiver(&machine_stash));

        // 问题5 修复（对齐 terminating-rental 的 S2 容错）：
        // 若矿工设置的收租钱包无法收款（如被冻结 / 低于存活额），回退到把这部分租金转给
        // stash，避免一个坏的 receiver 配置阻塞 confirm_rent / relet。
        // effective_payout_to 记录这笔钱真实落到哪，驱动后面的补质押门。
        let mut effective_payout_to = rent_receiver.clone();
        // NOTE: no-double-pay relies on Currency::transfer (pallet_balances) being
        // atomic-on-error (no partial debit). Revisit if the Currency type changes.
        let primary =
            <T as pallet::Config>::Currency::transfer(renter, &rent_receiver, fee_to_stash, KeepAlive);
        if primary.is_err() {
            if rent_receiver != machine_stash {
                // Transfer to stash first; only emit the fallback event once it succeeds.
                <T as pallet::Config>::Currency::transfer(
                    renter,
                    &machine_stash,
                    fee_to_stash,
                    KeepAlive,
                )?;
                effective_payout_to = machine_stash.clone();
                Self::deposit_event(Event::RentReceiverPayoutFallback(
                    machine_stash.clone(),
                    rent_receiver.clone(),
                    fee_to_stash,
                ));
            } else {
                // receiver 本就是 stash 仍失败 → 真失败，冒泡（外层 extrinsic 会回滚）
                primary?;
            }
        }
        if let Some(ref rent_fee_pot) = maybe_pot {
            if !fee_to_destroy.is_zero() {
                <T as pallet::Config>::Currency::transfer(
                    renter,
                    rent_fee_pot,
                    fee_to_destroy,
                    KeepAlive,
                )?;
            }
        }
        // 问题4 修复：不再用 let _ = 静默吞掉记账/补质押错误（资金已转移，记账失败需可观测）。
        // 不回滚（避免因补质押 reserve 失败阻塞已成功的租金支付），仅发事件。
        if T::RTOps::change_machine_rent_fee(
            machine_stash,
            machine_id,
            fee_to_destroy,
            fee_to_stash,
            effective_payout_to,
        )
        .is_err()
        {
            Self::deposit_event(Event::RentFeeAccountingFailed(rent_id));
        }
        Ok(())
    }

    /// [Thread B ③ · 托管] 从托管账户付款给 `to`；直转失败(受款方被冻结/低于ED)→记入 PendingDbcPayout 暂存，
    /// 受款方自行 claim_dbc_payout 领取。保证 settle_escrow 永不因单个受款方问题 revert 卡死。
    fn pay_from_escrow_or_defer(escrow: &T::AccountId, to: &T::AccountId, amount: BalanceOf<T>) {
        if amount.is_zero() {
            return
        }
        // AllowDeath（非 KeepAlive）：托管账户是 PalletId 派生账户，是「资金过路账户」。
        //   escrow 里同时持有所有在租订单的托管租金(Σ)，结算某单只取走该单那份。
        //   - 还有其它在租单 → 余额 = 其它单的 Σ ≥ ED，不会被 reap；
        //   - 这是最后一单 → 结算后应恰好归零(守恒: burn+miner_net+renter_refund==total)，
        //     KeepAlive 会因「不能把源账户打到 ED 以下」拒付最后一笔 → 静默 defer 到 PendingDbcPayout（bug）。
        //   故用 AllowDeath 允许把托管账户正常清空到 0。
        if <T as pallet::Config>::Currency::transfer(escrow, to, amount, AllowDeath).is_err() {
            PendingDbcPayout::<T>::mutate(to, |p| *p = p.saturating_add(amount));
            TotalPendingDbcPayout::<T>::mutate(|t| *t = t.saturating_add(amount));
            Self::deposit_event(Event::DbcPayoutDeferred(to.clone(), amount));
        }
    }

    /// [Thread B ③ · 托管] 结算一个托管订单，把托管的租金按已用时长分配。质押 bond 全程不碰。
    /// - `end_time`：结算时点。正常退租=rent_end；用户主动早退=now；离线早退=offline_time。
    /// - `offline`：是否因机器离线提前结算（决定是否罚矿工≤24h租金给租客）。用户主动早退 offline=false→penalty=0。
    /// 老单(无 EscrowedFee，已按老规则即时付)→ no-op。守恒：burn+miner_net+renter_refund==total_fee。
    fn settle_escrow(rent_id: RentOrderId, end_time: T::BlockNumber, offline: bool) -> DispatchResult {
        let total_fee = EscrowedFee::<T>::get(rent_id);
        if total_fee.is_zero() {
            return Ok(()) // 老单/已结算：无托管
        }
        let rent_info = match Self::rent_info(&rent_id) {
            Some(r) => r,
            None => {
                // 异常：订单已清但托管未结（不应发生）。清标记，钱留托管账户待 rescue，不静默丢。
                EscrowedFee::<T>::remove(rent_id);
                RentEscrowDestroyPercent::<T>::remove(rent_id);
                Self::deposit_event(Event::RentFeeAccountingFailed(rent_id));
                return Ok(())
            },
        };
        let escrow = Self::escrow_account();
        // [审计修 L1] 机器已被移除(如 force_machine_exit)但托管未结算：无矿工可付 → 全额退租客(best-effort)，
        //   清标记 + 发事件。原 `.ok_or(Unknown)?` 会让 settle_escrow 返回 Err，被 settle_and_finalize_rent
        //   的 `let _ =` 吞掉后仍清 RentInfo → 托管资金永久冻结在共享托管账户、无人可取。
        let machine_stash = match <online_profile::Pallet<T>>::machines_info(&rent_info.machine_id) {
            Some(mi) => mi.machine_stash,
            None => {
                Self::pay_from_escrow_or_defer(&escrow, &rent_info.renter, total_fee);
                EscrowedFee::<T>::remove(rent_id);
                RentEscrowDestroyPercent::<T>::remove(rent_id);
                Self::deposit_event(Event::RentFeeAccountingFailed(rent_id));
                return Ok(())
            },
        };
        let receiver = Self::rent_order_receiver(&rent_id)
            .unwrap_or_else(|| <online_profile::Pallet<T>>::effective_rent_receiver(&machine_stash));
        let destroy_percent = Self::rent_escrow_destroy_percent(&rent_id)
            .unwrap_or_else(|| <online_profile::Pallet<T>>::rent_fee_destroy_percent());

        // 已用比例（rent_start→rent_end 为计费窗口，与现有 rent_duration 口径一致）
        let duration = rent_info.rent_end.saturating_sub(rent_info.rent_start);
        let clamped_end = if end_time > rent_info.rent_end { rent_info.rent_end } else { end_time };
        let elapsed = clamped_end.saturating_sub(rent_info.rent_start);
        let dur_u32 = duration.saturated_into::<u32>();
        let ela_u32 = elapsed.saturated_into::<u32>();

        let used_fee = if dur_u32 == 0 {
            total_fee
        } else {
            Perbill::from_rational(ela_u32, dur_u32) * total_fee
        };
        let unused_fee = total_fee.saturating_sub(used_fee);
        let burn = destroy_percent * used_fee;
        let miner_gross = used_fee.saturating_sub(burn);
        let penalty = if offline {
            let one_day_u32 = ONE_DAY.saturated_into::<u32>();
            let fee_24h = if dur_u32 == 0 {
                Zero::zero()
            } else {
                Perbill::from_rational(one_day_u32.min(dur_u32), dur_u32) * total_fee
            };
            if fee_24h < miner_gross { fee_24h } else { miner_gross }
        } else {
            Zero::zero()
        };
        let miner_net = miner_gross.saturating_sub(penalty);
        let renter_refund = unused_fee.saturating_add(penalty);

        // 支付（全部从托管账户出；burn 若无 pot 则折进矿工，对齐 pay_rent_fee 的优雅回退）。
        // [审计修 L4] 记账须用「实际」销毁/给矿工额：无 pot 时 burn 并未销毁而是折进矿工，
        //   若仍把 burn 记成 fee_to_destroy 会产生幻影销毁统计 + 补质押基数偏小。故按分支返回实际值。
        let (eff_burn, eff_stash) = match Self::rent_fee_pot() {
            Some(pot) if !burn.is_zero() => {
                Self::pay_from_escrow_or_defer(&escrow, &pot, burn);
                Self::pay_from_escrow_or_defer(&escrow, &receiver, miner_net);
                (burn, miner_net)
            },
            _ => {
                // 无 pot：不销毁，burn 折进矿工（与 pay_rent_fee 一致）
                let to_miner = miner_net.saturating_add(burn);
                Self::pay_from_escrow_or_defer(&escrow, &receiver, to_miner);
                (Zero::zero(), to_miner)
            },
        };
        Self::pay_from_escrow_or_defer(&escrow, &rent_info.renter, renter_refund);

        // 生命周期租金记账 + 受收者门控的补质押（与旧 pay_rent_fee 尾部同一 RTOps 钩子）：
        //   - 累加 total_rent_fee / sys_info 计数（用 miner 实收 miner_net + 实销 burn）；
        //   - receiver == stash（未改收租钱包）且欠质押 → 从 stash 自有余额补质押（miner_net 刚打进 stash 自由余额）；
        //   - receiver != stash（改了收租钱包）→ 不补质押，防对 stash 双重扣款(2026-05-29 wukongyun 回归)。
        //   记账/补质押失败不回滚已成功的托管分账，仅发事件（对齐旧实现的可观测性）。
        if T::RTOps::change_machine_rent_fee(
            machine_stash,
            rent_info.machine_id.clone(),
            eff_burn,
            eff_stash,
            receiver.clone(),
        )
        .is_err()
        {
            Self::deposit_event(Event::RentFeeAccountingFailed(rent_id));
        }

        EscrowedFee::<T>::remove(rent_id);
        RentEscrowDestroyPercent::<T>::remove(rent_id);
        Self::deposit_event(Event::RentEscrowSettled(rent_id, used_fee, renter_refund, penalty));
        Ok(())
    }

    // 定时检查机器是否30分钟没有上线
    fn check_machine_starting_status(block_number: T::BlockNumber) -> Result<(), ()> {
        if !<ConfirmingOrder<T>>::contains_key(block_number) {
            return Ok(())
        }

        let pending_confirming = Self::confirming_order(block_number);
        for rent_id in pending_confirming {
            // [审计修 F-3] 单项容错：原 `ok_or(())?` 在某个 rent_info 缺失(异常态)时 abort 整批 →
            //   同块后续 rent_id 永不被处理(block 已过、on_finalize 只扫当前块) → 它们的 MachineRentedGPU
            //   卡 >0 → 守卫② 永久拒 DeepLink。改为：缺失则清掉 ConfirmingOrder 该项并继续，不拖累其他订单。
            let rent_info = match Self::rent_info(&rent_id) {
                Some(r) => r,
                None => {
                    let mut confirming_order = Self::confirming_order(block_number);
                    ItemList::rm_item(&mut confirming_order, &rent_id);
                    if confirming_order.is_empty() {
                        ConfirmingOrder::<T>::remove(block_number);
                    } else {
                        ConfirmingOrder::<T>::insert(block_number, confirming_order);
                    }
                    continue
                },
            };

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
            RentOrderReceiver::<T>::remove(rent_id);

            // [审计修 F-3] 单项容错：不让单个机器的状态变更失败 abort 整批（confirm_expired 现已保证计数落盘）
            let _ = T::RTOps::change_machine_status_on_confirm_expired(
                &rent_info.machine_id,
                rent_info.gpu_num,
            );
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
    /// [Thread B ③ · 托管] 结算 + 收尾一个租约订单（正常到期 / 租客主动早退 / 离线早退 三处共用，避免清理逻辑漂移）。
    /// - `settle_end_time`：结算时点（正常到期=rent_end；主动早退=now；离线早退=offline_time）。
    /// - `offline`：是否因机器离线提前结算（true 才罚矿工≤24h 租金给租客；主动早退/正常到期均 false=penalty0）。
    /// settle_escrow 先行（老单 no-op）；随后镜像原到期清理：改机器状态、退租客剩余质押、清 user_order /
    /// RentEnding[rent_end] / machine_rent_order / RentInfo / RentOrderReceiver。
    pub(crate) fn settle_and_finalize_rent(
        rent_id: RentOrderId,
        rent_info: &RentOrderDetail<T::AccountId, T::BlockNumber, BalanceOf<T>>,
        settle_end_time: T::BlockNumber,
        offline: bool,
    ) -> Result<(), ()> {
        let machine_id = rent_info.machine_id.clone();
        let clamped_end =
            if settle_end_time > rent_info.rent_end { rent_info.rent_end } else { settle_end_time };
        let rent_duration = clamped_end.saturating_sub(rent_info.rent_start);

        // 托管结算须在 RentInfo/receiver 清除前
        let _ = Self::settle_escrow(rent_id, settle_end_time, offline);

        // NOTE: 只要机器还有租用订单(>1)，就不修改成 online 状态。
        let is_last_rent = Self::is_last_rent(&machine_id, &rent_info.renter)?;
        let _ = T::RTOps::change_machine_status_on_rent_end(
            &machine_id,
            rent_info.gpu_num,
            rent_duration,
            is_last_rent.0,
            is_last_rent.1,
            rent_info.renter.clone(),
        );

        // 退还租客剩余质押（confirm 后一般为 0，保险处理）
        if !rent_info.stake_amount.is_zero() {
            let _ =
                Self::change_renter_total_stake(&rent_info.renter, rent_info.stake_amount, false);
        }

        let mut user_order = Self::user_order(&rent_info.renter);
        ItemList::rm_item(&mut user_order, &rent_id);
        if user_order.is_empty() {
            UserOrder::<T>::remove(&rent_info.renter);
        } else {
            UserOrder::<T>::insert(&rent_info.renter, user_order);
        }

        // 从订单自己的 rent_end 块的 RentEnding 里摘掉（正常到期时 == 当前块；早退时是未来块，摘掉防 on_finalize 重复处理）
        let mut rent_ending = Self::rent_ending(rent_info.rent_end);
        ItemList::rm_item(&mut rent_ending, &rent_id);
        if rent_ending.is_empty() {
            RentEnding::<T>::remove(rent_info.rent_end);
        } else {
            RentEnding::<T>::insert(rent_info.rent_end, rent_ending);
        }

        let mut machine_rent_order = Self::machine_rent_order(&rent_info.machine_id);
        machine_rent_order.clean_expired_order(rent_id, rent_info.gpu_index.clone());
        MachineRentOrder::<T>::insert(&rent_info.machine_id, machine_rent_order);

        RentInfo::<T>::remove(rent_id);
        RentOrderReceiver::<T>::remove(rent_id);
        Ok(())
    }

    fn check_if_rent_finished(block_number: T::BlockNumber) -> Result<(), ()> {
        if !<RentEnding<T>>::contains_key(block_number) {
            return Ok(())
        }

        let pending_ending = Self::rent_ending(block_number);
        for rent_id in pending_ending {
            // [审计 F-3 容错] 单个坏 rent_id 不阻断整批（对齐 check_machine_starting_status）
            let rent_info = match Self::rent_info(&rent_id) {
                Some(r) => r,
                None => continue,
            };
            // [Thread B ③] 正常到期：offline=false、settle_end=rent_end → used=100%、penalty=0（老单 no-op）
            let _ = Self::settle_and_finalize_rent(rent_id, &rent_info, rent_info.rent_end, false);
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

// [Thread B ③ · 离线终止] online-profile 的健康检测器路径调用：某在租机器被 DDN 报离线时，
// 结算并终止该机全部在租订单（offline=true → settle_escrow 罚≤24h 给租客；stake bond 不碰）。
// 机器状态由 online-profile 侧的 machine_offline 负责（已置 StakerReportOffline + 回退租用快照）；
// settle_and_finalize_rent 内的 change_machine_status_on_rent_end 因机器已处离线态而走离线分支：
// 只记 RentedFinished + 递减 MachineRentedGPU、**绝不二次回退快照**（复用已验证的"到期在离线态"机制）。
// 重新上线时 controller_report_online 见 RentedFinished → 机器回 Online（不复租）。
impl<T: Config> dbc_support::traits::RentTerminateOnOffline for Pallet<T> {
    type MachineId = MachineId;

    fn settle_terminate_rents_on_offline(machine_id: &MachineId) {
        let now = <frame_system::Pallet<T>>::block_number();
        // 迭代快照：settle_and_finalize_rent 会改 MachineRentOrder，用本地副本迭代避免边改边读。
        let machine_order = Self::machine_rent_order(machine_id);
        for rent_id in machine_order.rent_order.iter() {
            let rent_info = match Self::rent_info(rent_id) {
                Some(r) => r,
                None => continue,
            };
            // 只终止已确认在租(Renting)的订单：WaitingVerifying 无托管、由 confirm 超时清理，勿在此误结算。
            if rent_info.rent_status != RentStatus::Renting {
                continue
            }
            // [审计修 MED/round2 · grandfather] 跳过升级前老单（无 EscrowedFee）：老单钱已在 confirm 即时付给矿工、
            //   settle_escrow 是 no-op，若在此强制终止 → 租客白丢剩余预付天数（且矿工可自杀式重租套利）。老单按老
            //   规则走自然到期(check_if_rent_finished)。对齐 DESIGN_LOCK_3A_escrow.md「legacy 老单 finish under old rules」。
            if !EscrowedFee::<T>::contains_key(*rent_id) {
                continue
            }
            // best-effort：单个订单结算失败不冒泡（离线转换已在 online-profile 侧完成，勿因某单拖垮）。
            let _ = Self::settle_and_finalize_rent(*rent_id, &rent_info, now, true);
        }
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
        let extra_price = <online_profile::Pallet<T>>::machine_extra_price(&machine_id)
            .checked_mul(rent_gpu_num as u64).ok_or(Error::<T>::Overflow)?;
        let machine_price = system_price.checked_add(extra_price).ok_or(Error::<T>::Overflow)?;

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
        let extra_price = <online_profile::Pallet<T>>::machine_extra_price(&machine_id)
            .checked_mul(rent_gpu_num as u64).ok_or(Error::<T>::Overflow)?;
        let machine_price = system_price.checked_add(extra_price).ok_or(Error::<T>::Overflow)?;

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
        let extra_price = <online_profile::Pallet<T>>::machine_extra_price(&machine_id)
            .checked_mul(rent_gpu_num as u64).ok_or(Error::<T>::Overflow)?;
        let machine_price = system_price.checked_add(extra_price).ok_or(Error::<T>::Overflow)?;

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
