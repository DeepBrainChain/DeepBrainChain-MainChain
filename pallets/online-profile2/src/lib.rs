#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

mod types;

use dbc_support::{
    live_machine::LiveMachine,
    machine_info::MachineInfo,
    machine_type::{Latitude, Longitude, MachineStatus, StakerCustomizeInfo},
    traits::{DbcPrice, GNOps, ManageCommittee},
    verify_online::StashMachine,
    verify_slash::{OPPendingSlashInfo, OPPendingSlashReviewInfo, OPSlashReason},
    EraIndex, ItemList, MachineId, SlashId, ONE_DAY,
};
use frame_support::{
    pallet_prelude::*,
    traits::{Currency, EnsureOrigin, OnUnbalanced, Randomness, ReservableCurrency},
    weights::Weight,
};
use frame_system::pallet_prelude::*;

type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T> = <<T as Config>::Currency as Currency<
    <T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

pub use pallet::*;
pub use types::*;

#[frame_support::pallet]
pub mod pallet {
    use sp_runtime::Perbill;

    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + generic_func::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Currency: ReservableCurrency<Self::AccountId>;
        type BondingDuration: Get<EraIndex>;
        type DbcPrice: DbcPrice<Balance = BalanceOf<Self>>;
        type ManageCommittee: ManageCommittee<
            AccountId = Self::AccountId,
            Balance = BalanceOf<Self>,
        >;
        type Slash: OnUnbalanced<NegativeImbalanceOf<Self>>;
        type CancelSlashOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        type SlashAndReward: GNOps<AccountId = Self::AccountId, Balance = BalanceOf<Self>>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    #[pallet::getter(fn phase_reward_info)]
    pub(super) type PhaseRewardInfo<T: Config> =
        StorageValue<_, PhaseRewardInfoDetail<BalanceOf<T>>>;

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// When reward start to distribute
        #[pallet::weight(0)]
        pub fn set_reward_info(
            origin: OriginFor<T>,
            reward_info: PhaseRewardInfoDetail<BalanceOf<T>>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            <PhaseRewardInfo<T>>::put(reward_info);
            Ok(().into())
        }
    }

    #[pallet::event]
    // #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {}

    #[pallet::error]
    pub enum Error<T> {}
}
