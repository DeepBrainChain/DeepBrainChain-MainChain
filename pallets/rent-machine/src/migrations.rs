use crate::{
    Config, Module, PendingConfirming, PendingRentEnding, RentOrder, RentOrderDetail, RentStatus, StorageVersion,
    UserRented, WAITING_CONFIRMING_DELAY,
};
use codec::{Decode, Encode};
use frame_support::{debug::info, traits::Get, weights::Weight, IterableStorageMap};
use generic_func::{ItemList, MachineId};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::traits::Zero;
use sp_std::vec::Vec;

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

    if StorageVersion::<T>::get() <= 1 {
        // NOTE: Update storage version.
        StorageVersion::<T>::put(2);

        clean_expired_storage::<T>().saturating_add(migrate_rent_order_to_v2::<T>())
    } else {
        frame_support::debug::info!(" >>> Unused migration!");
        0
    }
}

fn clean_expired_storage<T: Config>() -> Weight {
    // NOTE: 清理过期存储: PendingRentEnding; PendingConfirming
    let now = <frame_system::Module<T>>::block_number();
    let pending_rent_ending: Vec<T::BlockNumber> =
        <PendingRentEnding<T> as IterableStorageMap<T::BlockNumber, _>>::iter()
            .map(|(time, _)| time)
            .collect::<Vec<_>>();
    for time in pending_rent_ending {
        if time < now {
            <PendingRentEnding<T>>::remove(time);
        }
    }

    // TODO: 对于所有的pending_confirming，由 RentOrderId -> T::AccountId 改为了
    // T::BlockNumber -> Vec<RentOrderId>
    let pending_confirming: Vec<T::BlockNumber> =
        <PendingConfirming<T> as IterableStorageMap<T::BlockNumber, _>>::iter()
            .map(|(time, _)| time)
            .collect::<Vec<_>>();
    for time in pending_confirming {
        if time < now {
            <PendingConfirming<T>>::remove(time)
        }
    }

    0
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

// 根据OldRentOrder生成新的RentOrder, UserRented
fn migrate_rent_order_to_v2<T: Config>() -> Weight {
    let all_rent_order: Vec<MachineId> = <deprecated::RentOrder<T> as IterableStorageMap<MachineId, _>>::iter()
        .map(|(machine_id, _)| machine_id)
        .collect::<Vec<_>>();

    for machine_id in all_rent_order {
        let rent_order = <deprecated::Module<T>>::rent_order(&machine_id);
        let rent_order_id = <Module<T>>::get_new_rent_id();
        let machine_info = <online_profile::Module<T>>::machines_info(&machine_id);
        RentOrder::<T>::insert(
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

        let mut user_rented = <Module<T>>::user_rented(&rent_order.renter);
        ItemList::add_item(&mut user_rented, rent_order_id);
        UserRented::<T>::insert(rent_order.renter, user_rented);

        if rent_order.confirm_rent.is_zero() && !rent_order.rent_start.is_zero() {
            let confirming_expire_at = rent_order.rent_start + WAITING_CONFIRMING_DELAY.into();
            let mut pending_confirming = <Module<T>>::pending_confirming(confirming_expire_at);
            ItemList::add_item(&mut pending_confirming, rent_order_id);
            PendingConfirming::<T>::insert(confirming_expire_at, pending_confirming);
        }

        let mut pending_rent_ending = <Module<T>>::pending_rent_ending(rent_order.rent_end);
        ItemList::add_item(&mut pending_rent_ending, rent_order_id);
        PendingRentEnding::<T>::insert(rent_order.rent_end, pending_rent_ending);

        // TODO: 删除
        // <deprecated::RentOrder<Module<T>>>::contains_key(machine_id);
        // NOTE: 在该变量迁移完成之前不能删除
        // <derepcated::UserRented<Module<T>>::remove(rent_order.renter);
        // TODO: 删除deprecated::PendingRentEnding
    }

    let count = RentOrder::<T>::iter_values().count();

    info!(
        target: "runtime::rentMachine",
        "migrated {} rentMachine::RentOrder, UserRented, PendingRentEnding, PendingConfirming",
        count,
    );

    <T as frame_system::Config>::DbWeight::get().reads_writes(count as Weight + 1, count as Weight + 1)
}
