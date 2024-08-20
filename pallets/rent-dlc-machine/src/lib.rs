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
    rental_type::{MachineGPUOrder, MachineRentedOrderDetail, RentOrderDetail, RentStatus},
    traits::{DLCMachineInfoTrait, DbcPrice, RTOps},
    EraIndex, ItemList, MachineId, RentOrderId, ONE_DAY,
};
use frame_support::{
    ensure,
    pallet_prelude::*,
    traits::tokens::fungibles::{Inspect, Mutate},
};
use frame_system::{ensure_signed, pallet_prelude::*};
use sp_runtime::{
    traits::{CheckedAdd, SaturatedConversion, Saturating, Zero},
    Perbill,
};
use sp_std::{prelude::*, vec::Vec};

type BalanceOf<T> = <T as pallet_assets::Config>::Balance;
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + online_profile::Config
        + rent_machine::Config
        + pallet_assets::Config
        + dlc_machine::Config
    {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type RTOps: RTOps<
            MachineId = MachineId,
            MachineStatus = MachineStatus<Self::BlockNumber, Self::AccountId>,
            AccountId = Self::AccountId,
            Balance = BalanceOf<Self>,
            BlockNumber = Self::BlockNumber,
        >;
        type DbcPrice: DbcPrice<Balance = BalanceOf<Self>>;
        type AssetId: IsType<<Self as pallet_assets::Config>::AssetId> + Parameter + From<u32>;

        #[pallet::constant]
        type DLCAssetId: Get<u32>;
    }

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(_block_number: T::BlockNumber) {
            let _ = Self::check_if_rent_finished();
        }
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

    #[pallet::storage]
    #[pallet::getter(fn next_rent_id)]
    pub(super) type NextRentId<T: Config> = StorageValue<_, RentOrderId, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn rent_info)]
    pub type RentInfo<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        RentOrderId,
        RentOrderDetail<T::AccountId, T::BlockNumber, BalanceOf<T>>,
    >;

    #[pallet::storage]
    #[pallet::getter(fn dlc_machine_rented_gpu)]
    pub type DLCMachineRentedGPU<T: Config> =
        StorageMap<_, Blake2_128Concat, MachineId, u32, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn rent_ending)]
    pub(super) type RentEnding<T: Config> =
        StorageMap<_, Blake2_128Concat, T::BlockNumber, Vec<RentOrderId>, ValueQuery>;

    #[pallet::type_value]
    pub(super) fn MaximumRentalDurationDefault<T: Config>() -> EraIndex {
        60
    }

    #[pallet::storage]
    #[pallet::getter(fn maximum_rental_duration)]
    pub(super) type MaximumRentalDuration<T: Config> =
        StorageValue<_, EraIndex, ValueQuery, MaximumRentalDurationDefault<T>>;

    #[pallet::storage]
    #[pallet::getter(fn burn_total_amount)]
    pub(super) type BurnTotalAmount<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn burn_records)]
    pub(super) type BurnRecords<T: Config> =
        StorageValue<_, Vec<(BalanceOf<T>, T::BlockNumber, T::AccountId, RentOrderId)>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn dlc_rent_id_2_parent_dbc_rent_id)]
    pub type DlcRentId2ParentDbcRentId<T: Config> =
        StorageMap<_, Blake2_128Concat, RentOrderId, RentOrderId>;

    #[pallet::storage]
    #[pallet::getter(fn machine_rented_orders)]
    pub type MachineRentedOrders<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        MachineId,
        Vec<MachineRentedOrderDetail<T::AccountId, T::BlockNumber>>,
        ValueQuery,
    >;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(10000, 0))]
        pub fn rent_dlc_machine(
            origin: OriginFor<T>,
            machine_id: MachineId,
            rent_gpu_num: u32,
            duration: T::BlockNumber,
        ) -> DispatchResultWithPostInfo {
            let renter = ensure_signed(origin.clone())?;
            Self::rent_machine_by_block(renter, machine_id, rent_gpu_num, duration)
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        PayTxFeeAndBurn(RentOrderId, T::AccountId, BalanceOf<T>),
        DLCRent(RentOrderId, T::AccountId, MachineId, u32, T::BlockNumber, BalanceOf<T>),
    }

    #[pallet::error]
    pub enum Error<T> {
        MachineNotRentable,
        Overflow,
        GetMachinePriceFailed,
        OnlyHalfHourAllowed,
        GPUNotEnough,
        MachineNotFound,

        DBCMachineRenterNotOwner,
        MachineOwnerNotRentAllGPU,
        DBCMachineNotRentedByOwner,
        MachineNotDLCStaking,
        MappingDlcRentId2ParentDbcRentIdFailed,
        DLCRentIdNotFound,
        DBCRentIdNotFound,
    }

    use dbc_support::rental_type::MachineRentedOrderDetail;
}

impl<T: Config> Pallet<T> {
    fn rent_machine_by_block(
        renter: T::AccountId,
        machine_id: MachineId,
        rent_gpu_num: u32,
        duration: T::BlockNumber,
    ) -> DispatchResultWithPostInfo {
        let found = dlc_machine::Pallet::<T>::dlc_machine_in_staking(machine_id.clone());
        ensure!(found, Error::<T>::MachineNotDLCStaking);

        let now = <frame_system::Pallet<T>>::block_number();
        let machine_info = <online_profile::Pallet<T>>::machines_info(&machine_id)
            .ok_or(Error::<T>::MachineNotFound)?;
        let machine_rented_gpu = Self::dlc_machine_rented_gpu(&machine_id);
        let gpu_num = machine_info.gpu_num();

        if gpu_num == 0 || duration == Zero::zero() {
            return Ok(().into())
        }

        let dbc_machine_rent_order_info =
            <rent_machine::Pallet<T>>::machine_rent_order(&machine_id);
        ensure!(
            dbc_machine_rent_order_info.rent_order.len() == 1,
            Error::<T>::DBCMachineRenterNotOwner
        );

        let dbc_rent_order_id = dbc_machine_rent_order_info.rent_order[0];
        let dbc_machine_rent_info = <rent_machine::Pallet<T>>::rent_info(&dbc_rent_order_id)
            .ok_or(Error::<T>::DBCMachineNotRentedByOwner)?;

        ensure!(
            dbc_machine_rent_info.renter == machine_info.controller,
            Error::<T>::DBCMachineRenterNotOwner
        );
        ensure!(
            dbc_machine_rent_info.gpu_num == machine_info.gpu_num(),
            Error::<T>::MachineOwnerNotRentAllGPU
        );

        // check free GPU number
        ensure!(rent_gpu_num + machine_rented_gpu <= gpu_num, Error::<T>::GPUNotEnough);

        // rent duration must be 30min * n
        ensure!(duration % 60u32.into() == Zero::zero(), Error::<T>::OnlyHalfHourAllowed);

        // 检查machine_id状态是否可以租用
        ensure!(
            machine_info.machine_status == MachineStatus::Rented,
            Error::<T>::MachineNotRentable
        );

        let max_duration = Self::get_dbc_rent_machine_duration(&machine_id)?;

        // rent duration must be less than the rent duration of owner(dbc machine)
        let duration = duration.min(max_duration);

        // get machine price
        let machine_price = <T as Config>::RTOps::get_machine_price(
            machine_info.calc_point(),
            rent_gpu_num,
            gpu_num,
        )
        .ok_or(Error::<T>::GetMachinePriceFailed)?;

        // calculate rent_fee
        let rent_fee_value = machine_price
            .checked_mul(duration.saturated_into::<u64>())
            .ok_or(Error::<T>::Overflow)?
            .checked_div(ONE_DAY as u64)
            .ok_or(Error::<T>::Overflow)?;
        let dbc_rent_fee = <T as Config>::DbcPrice::get_dbc_amount_by_value(rent_fee_value)
            .ok_or(Error::<T>::Overflow)?;

        // dlc rent machine fee should be 125% of dbc rent fee
        let rent_fee = Perbill::from_rational(25u32, 100u32) * dbc_rent_fee + dbc_rent_fee;

        // get rent end block number
        let rent_end = duration.checked_add(&now).ok_or(Error::<T>::Overflow)?;

        let rent_id = Self::get_new_rent_id();
        ensure!(
            Self::mapping_dlc_rent_id_2_parent_dbc_rent_id(&rent_id, &machine_id) == Ok(()),
            Error::<T>::MappingDlcRentId2ParentDbcRentIdFailed
        );

        let mut machine_rent_order = Self::machine_rent_order(&machine_id);
        let rentable_gpu_index = machine_rent_order.gen_rentable_gpu(rent_gpu_num, gpu_num);
        ItemList::add_item(&mut machine_rent_order.rent_order, rent_id);

        let asset_id = Self::get_dlc_asset_id();
        <pallet_assets::Pallet<T> as Inspect<T::AccountId>>::can_withdraw(
            asset_id.clone(),
            &renter,
            rent_fee.clone(),
        )
        .into_result(false)?;

        <pallet_assets::Pallet<T> as Mutate<T::AccountId>>::shelve(
            asset_id,
            &renter,
            rent_fee.clone(),
        )?;

        BurnRecords::<T>::mutate(|records| {
            records.push((
                rent_fee.clone(),
                <frame_system::Pallet<T>>::block_number(),
                renter.clone(),
                rent_id.clone(),
            ));
        });
        BurnTotalAmount::<T>::mutate(|total_amount| {
            *total_amount = total_amount.saturating_add(rent_fee.clone())
        });

        Self::deposit_event(Event::PayTxFeeAndBurn(rent_id, renter.clone(), rent_fee.clone()));

        DLCMachineRentedGPU::<T>::mutate(&machine_id, |rented_gpu| {
            *rented_gpu = rented_gpu.saturating_add(rent_gpu_num)
        });

        let mut rent_info = RentOrderDetail::new(
            machine_id.clone(),
            renter.clone(),
            now,
            rent_end,
            rent_fee.clone(),
            rent_gpu_num,
            rentable_gpu_index,
        );
        rent_info.rent_status = RentStatus::Renting;
        rent_info.confirm_rent = rent_info.rent_start;

        RentInfo::<T>::insert(&rent_id, rent_info);

        UserOrder::<T>::mutate(&renter, |user_order| {
            ItemList::add_item(user_order, rent_id);
        });

        RentEnding::<T>::mutate(rent_end, |rent_ending| {
            ItemList::add_item(rent_ending, rent_id);
        });

        MachineRentOrder::<T>::insert(&machine_id, machine_rent_order);

        MachineRentedOrders::<T>::mutate(&machine_id, |machine_rented_orders| {
            machine_rented_orders.push(MachineRentedOrderDetail {
                renter: renter.clone(),
                rent_start: now,
                rent_end,
                rent_id: rent_id.clone(),
            });
        });

        Self::deposit_event(Event::DLCRent(
            rent_id,
            renter,
            machine_id,
            rent_gpu_num,
            duration.into(),
            rent_fee,
        ));
        Ok(().into())
    }

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

    // -Write: MachineRentOrder, RentEnding, RentOrder,
    // UserOrder, ConfirmingOrder
    fn clean_order(who: &T::AccountId, rent_order_id: RentOrderId) -> Result<(), ()> {
        let mut user_order = Self::user_order(who);
        ItemList::rm_item(&mut user_order, &rent_order_id);

        let rent_info = Self::rent_info(rent_order_id).ok_or(())?;

        let mut rent_ending = Self::rent_ending(rent_info.rent_end);
        ItemList::rm_item(&mut rent_ending, &rent_order_id);

        let mut machine_rent_order = Self::machine_rent_order(&rent_info.machine_id);
        machine_rent_order.clean_expired_order(rent_order_id, rent_info.gpu_index);

        DLCMachineRentedGPU::<T>::mutate(&rent_info.machine_id, |rented_gpu| {
            *rented_gpu = rented_gpu.saturating_sub(rent_info.gpu_num)
        });

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
        Ok(())
    }

    // 这里修rentMachine模块通知onlineProfile机器已经租用完成，
    // onlineProfile判断机器是否需要变成online状态，或者记录下之前是租用状态，
    // 以便机器再次上线时进行正确的惩罚
    fn check_if_rent_finished() -> Result<(), ()> {
        let now = <frame_system::Pallet<T>>::block_number();
        if !<RentEnding<T>>::contains_key(now) {
            return Ok(())
        }
        let pending_ending = Self::rent_ending(now);

        for rent_id in pending_ending {
            let rent_info = Self::rent_info(&rent_id).ok_or(())?;

            let _ = Self::clean_order(&rent_info.renter, rent_id);
        }
        Ok(())
    }

    pub fn get_dlc_asset_id_parameter() -> <T as pallet_assets::Config>::AssetIdParameter {
        let asset_id: <T as Config>::AssetId = <T as Config>::DLCAssetId::get().into();
        asset_id.into().into()
    }

    pub fn get_dlc_asset_id() -> <pallet_assets::Pallet<T> as Inspect<T::AccountId>>::AssetId {
        let asset_id: <T as Config>::AssetId = <T as Config>::DLCAssetId::get().into();
        asset_id.into()
    }

    pub fn get_renters(machine_id: &MachineId) -> Vec<T::AccountId> {
        let rent_order_info = Self::machine_rent_order(machine_id);
        let mut renters: Vec<T::AccountId> = Vec::new();
        for rent_order_id in rent_order_info.rent_order {
            if let Some(rent_info) = Self::rent_info(rent_order_id) {
                renters.push(rent_info.renter);
            }
        }
        renters
    }

    fn mapping_dlc_rent_id_2_parent_dbc_rent_id(
        rent_id: &RentOrderId,
        machine_id: &MachineId,
    ) -> Result<(), ()> {
        if let Some(parent_dbc_rent_id) =
            rent_machine::Pallet::<T>::get_rent_id_of_renting_dbc_machine_by_owner(machine_id)
        {
            DlcRentId2ParentDbcRentId::<T>::insert(rent_id, parent_dbc_rent_id);
            return Ok(())
        }
        Err(())
    }

    fn get_dbc_rent_machine_duration(
        machine_id: &MachineId,
    ) -> Result<T::BlockNumber, DispatchError> {
        let rent_id =
            rent_machine::Pallet::<T>::get_rent_id_of_renting_dbc_machine_by_owner(machine_id)
                .ok_or(Error::<T>::DBCRentIdNotFound)?;

        let dbc_rent_info =
            rent_machine::Pallet::<T>::rent_info(rent_id).ok_or(Error::<T>::DBCRentIdNotFound)?;

        Ok(dbc_rent_info.rent_end.saturating_sub(dbc_rent_info.rent_start))
    }

    pub fn get_parent_dbc_rent_order_id(
        rent_id: RentOrderId,
    ) -> Result<RentOrderId, DispatchError> {
        let _ = Self::rent_info(rent_id).ok_or(Error::<T>::DLCRentIdNotFound)?;
        let dbc_rent_id =
            Self::dlc_rent_id_2_parent_dbc_rent_id(rent_id).ok_or(Error::<T>::DLCRentIdNotFound)?;

        Ok(dbc_rent_id)
    }
}

impl<T: Config> DLCMachineInfoTrait for Pallet<T> {
    type BlockNumber = T::BlockNumber;
    fn get_dlc_machine_rent_duration(
        last_claim_at: T::BlockNumber,
        slash_at: T::BlockNumber,
        machine_id: MachineId,
    ) -> Result<T::BlockNumber, &'static str> {
        let now = <frame_system::Pallet<T>>::block_number();
        let mut rent_duration: T::BlockNumber = T::BlockNumber::default();
        let rented_orders = Self::machine_rented_orders(machine_id);
        if slash_at == T::BlockNumber::default() {
            rented_orders.iter().for_each(|rented_order| {
                if rented_order.rent_end >= last_claim_at {
                    if rented_order.rent_end >= now {
                        rent_duration += now - last_claim_at
                    } else {
                        rent_duration += rented_order.rent_end - last_claim_at
                    }
                }
            });
        } else {
            rented_orders.iter().for_each(|rented_order| {
                if rented_order.rent_end >= last_claim_at {
                    if rented_order.rent_end >= slash_at && slash_at >= last_claim_at {
                        rent_duration += slash_at - last_claim_at;
                    } else if rented_order.rent_end < slash_at &&
                        rented_order.rent_end >= last_claim_at
                    {
                        rent_duration += rented_order.rent_end - last_claim_at
                    }
                }
            });
        }

        Ok(rent_duration)
    }
}
