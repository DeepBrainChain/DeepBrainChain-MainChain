use crate::{
    Config, ConfirmingOrder, MachineGPUOrder, MachineRentOrder, Module, Pallet, RentEnding,
    RentInfo, StorageVersion, UserOrder, WAITING_CONFIRMING_DELAY,
};
use codec::{Decode, Encode};
use dbc_support::{
    rental_type::{RentOrderDetail, RentStatus},
    traits::RTOps,
    ItemList, MachineId,
};
use frame_support::{debug::info, traits::Get, weights::Weight, IterableStorageMap};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::traits::Zero;
use sp_std::{vec, vec::Vec};

/// Apply all of the migrations due to taproot.
///
/// ### Warning
///
/// Use with care and run at your own risk.
pub fn apply<T: Config>() -> Weight {
    frame_support::debug::RuntimeLogger::init();

    info!(
        target: "runtime::rent_machine",
        "Running migration for rentMachine pallet"
    );

    let storage_version = StorageVersion::<T>::get();

    if storage_version <= 1 {
        // NOTE: Update storage version.
        StorageVersion::<T>::put(2);
        migrate_rent_order_to_v2::<T>()
    } else if storage_version == 2 {
        StorageVersion::<T>::put(3);
        fix_online_machine_renters::<T>()
    } else {
        frame_support::debug::info!(" >>> Unused migration!");
        0
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct OldRentOrderDetail<AccountId, BlockNumber, Balance> {
    pub renter: AccountId,
    pub rent_start: BlockNumber,
    pub confirm_rent: BlockNumber,
    pub rent_end: BlockNumber,
    pub stake_amount: Balance,
    pub rent_status: RentStatus,
}

pub mod deprecated {
    use crate::{migrations::OldRentOrderDetail, BalanceOf, Config, MachineId};
    use frame_support::{decl_module, decl_storage};
    use sp_std::prelude::*;

    decl_storage! {
        trait Store for Module<T: Config> as RentMachine {
            // 存储用户当前租用的机器列表
            pub UserRented get(fn user_rented): map hasher(blake2_128_concat)
                T::AccountId => Vec<MachineId>;

            // 用户租用的某个机器的详情
            pub RentOrder get(fn rent_order): map hasher(blake2_128_concat)
                MachineId => OldRentOrderDetail<T::AccountId, T::BlockNumber, BalanceOf<T>>;

            // 等待用户确认租用成功的机器
            pub PendingConfirming get(fn pending_confirming): map hasher(blake2_128_concat)
                MachineId => T::AccountId;

            // 记录每个区块将要结束租用的机器
            pub PendingRentEnding get(fn pending_rent_ending): map hasher(blake2_128_concat)
                T::BlockNumber => Vec<MachineId>;
        }
    }
    decl_module! {
        pub struct Module<T: Config> for enum Call where origin: T::Origin { }
    }
}

// 根据OldRentOrder生成新的RentOrder, UserOrder
fn migrate_rent_order_to_v2<T: Config>() -> Weight {
    let all_rent_order: Vec<MachineId> =
        <deprecated::RentOrder<T> as IterableStorageMap<MachineId, _>>::iter()
            .map(|(machine_id, _)| machine_id)
            .collect::<Vec<_>>();

    for machine_id in all_rent_order {
        let rent_order = <deprecated::Module<T>>::rent_order(&machine_id);
        let rent_order_id = <Module<T>>::get_new_rent_id();
        let machine_info = <online_profile::Module<T>>::machines_info(&machine_id);
        RentInfo::<T>::insert(
            rent_order_id,
            RentOrderDetail {
                machine_id: machine_info.machine_id(),
                renter: rent_order.renter.clone(),
                rent_start: rent_order.rent_start,
                confirm_rent: rent_order.confirm_rent,
                rent_end: rent_order.rent_end,
                stake_amount: rent_order.stake_amount,
                rent_status: rent_order.rent_status,
                gpu_num: machine_info.gpu_num(),
                gpu_index: (0..machine_info.gpu_num()).collect(),
            },
        );

        let mut user_order = <Module<T>>::user_order(&rent_order.renter);
        ItemList::add_item(&mut user_order, rent_order_id);
        UserOrder::<T>::insert(rent_order.renter, user_order);

        MachineRentOrder::<T>::insert(
            machine_info.machine_id(),
            MachineGPUOrder {
                rent_order: vec![rent_order_id],
                used_gpu: (0..machine_info.gpu_num()).collect(),
            },
        );

        if rent_order.confirm_rent.is_zero() && !rent_order.rent_start.is_zero() {
            let confirming_expire_at =
                rent_order.rent_start + (2 * WAITING_CONFIRMING_DELAY).into();
            let mut pending_confirming = <Module<T>>::confirming_order(confirming_expire_at);
            ItemList::add_item(&mut pending_confirming, rent_order_id);
            ConfirmingOrder::<T>::insert(confirming_expire_at, pending_confirming);
        } else {
            let mut rent_ending = <Module<T>>::rent_ending(rent_order.rent_end);
            ItemList::add_item(&mut rent_ending, rent_order_id);
            RentEnding::<T>::insert(rent_order.rent_end, rent_ending);
        }
    }

    let count = RentInfo::<T>::iter_values().count();

    info!(
        target: "runtime::rentMachine",
        "migrated {} rentMachine::RentInfo, MachineRentOrder, UserOrder, PendingRentEnding, PendingConfirming",
        count,
    );

    <T as frame_system::Config>::DbWeight::get()
        .reads_writes(count as Weight * 5 + 1, count as Weight * 5 + 1)
}

// 修复第一次迁移时机器将last_machine_rentern当作renters而未考虑机器状态的问题
fn fix_online_machine_renters<T: Config>() -> Weight {
    let all_machine_id = <MachineRentOrder<T> as IterableStorageMap<MachineId, _>>::iter()
        .map(|(machine_id, _)| machine_id)
        .collect::<Vec<_>>();

    for machine_id in all_machine_id {
        let machine_rent_order = Pallet::<T>::machine_rent_order(&machine_id);
        let mut renters = vec![];
        for rent_id in machine_rent_order.rent_order {
            let rent_info = Pallet::<T>::rent_info(rent_id);
            ItemList::add_item(&mut renters, rent_info.renter);
        }
        T::RTOps::reset_machine_renters(machine_id, renters);
    }
    0
}
