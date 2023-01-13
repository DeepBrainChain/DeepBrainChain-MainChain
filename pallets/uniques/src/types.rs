//! Various basic types for use in the Uniques pallet.

use super::*;
use scale_info::TypeInfo;

pub(super) type DepositBalanceOf<T = ()> =
    <<T as Config>::Currency as Currency<<T as SystemConfig>::AccountId>>::Balance;
pub(super) type CollectionDetailsFor<T> =
    CollectionDetails<<T as SystemConfig>::AccountId, DepositBalanceOf<T>>;
pub(super) type ItemDetailsFor<T> =
    ItemDetails<<T as SystemConfig>::AccountId, DepositBalanceOf<T>>;
pub(super) type ItemPrice<T = ()> =
    <<T as Config>::Currency as Currency<<T as SystemConfig>::AccountId>>::Balance;

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct CollectionDetails<AccountId, DepositBalance> {
    /// Can change `owner`, `issuer`, `freezer` and `admin` accounts.
    pub(super) owner: AccountId,
    /// Can mint tokens.
    pub(super) issuer: AccountId,
    /// Can thaw tokens, force transfers and burn tokens from any account.
    pub(super) admin: AccountId,
    /// Can freeze tokens.
    pub(super) freezer: AccountId,
    /// The total balance deposited for the all storage associated with this collection.
    /// Used by `destroy`.
    pub(super) total_deposit: DepositBalance,
    /// If `true`, then no deposit is needed to hold items of this collection.
    pub(super) free_holding: bool,
    /// The total number of outstanding items of this collection.
    pub(super) items: u32,
    /// The total number of outstanding item metadata of this collection.
    pub(super) item_metadatas: u32,
    /// The total number of attributes for this collection.
    pub(super) attributes: u32,
    /// Whether the collection is frozen for non-admin transfers.
    pub(super) is_frozen: bool,
}

/// Witness data for the destroy transactions.
#[derive(Copy, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct DestroyWitness {
    /// The total number of outstanding items of this collection.
    #[codec(compact)]
    pub items: u32,
    /// The total number of items in this collection that have outstanding item metadata.
    #[codec(compact)]
    pub item_metadatas: u32,
    #[codec(compact)]
    /// The total number of attributes for this collection.
    pub attributes: u32,
}

impl<AccountId, DepositBalance> CollectionDetails<AccountId, DepositBalance> {
    pub fn destroy_witness(&self) -> DestroyWitness {
        DestroyWitness {
            items: self.items,
            item_metadatas: self.item_metadatas,
            attributes: self.attributes,
        }
    }
}

/// Information concerning the ownership of a single unique item.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct ItemDetails<AccountId, DepositBalance> {
    /// The owner of this item.
    pub(super) owner: AccountId,
    /// The approved transferrer of this item, if one is set.
    pub(super) approved: Option<AccountId>,
    /// Whether the item can be transferred or not.
    pub(super) is_frozen: bool,
    /// The amount held in the pallet's default account for this item. Free-hold items will have
    /// this as zero.
    pub(super) deposit: DepositBalance,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
#[scale_info(skip_type_params(StringLimit))]
// #[codec(mel_bound(DepositBalance: MaxEncodedLen))]
pub struct CollectionMetadata<DepositBalance> {
    /// The balance deposited for this metadata.
    ///
    /// This pays for the data stored in this struct.
    pub(super) deposit: DepositBalance,
    /// General information concerning this collection. Limited in length by `StringLimit`. This
    /// will generally be either a JSON dump or the hash of some JSON which can be found on a
    /// hash-addressable global publication system such as IPFS.
    // NOTE: StringLimit
    pub(super) data: Vec<u8>,
    /// Whether the collection's metadata may be changed by a non Force origin.
    pub(super) is_frozen: bool,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
#[scale_info(skip_type_params(StringLimit))]
// #[codec(mel_bound(DepositBalance: MaxEncodedLen))]
pub struct ItemMetadata<DepositBalance> {
    /// The balance deposited for this metadata.
    ///
    /// This pays for the data stored in this struct.
    pub(super) deposit: DepositBalance,
    /// General information concerning this item. Limited in length by `StringLimit`. This will
    /// generally be either a JSON dump or the hash of some JSON which can be found on a
    /// hash-addressable global publication system such as IPFS.
    // NOTE: data: StringLimit
    pub(super) data: Vec<u8>,
    /// Whether the item metadata may be changed by a non Force origin.
    pub(super) is_frozen: bool,
}
