//! Implementations for `nonfungibles` traits.

use super::*;
use frame_support::{pallet_prelude::DispatchResultWithPostInfo, traits::Get};
use sp_std::prelude::*;

// impl Inspect
impl<T: Config> Pallet<T> {
    /// Returns the attribute value of `item` of `collection` corresponding to `key`.
    ///
    /// When `key` is empty, we return the item metadata value.
    ///
    /// By default this is `None`; no attributes are defined.
    fn attribute(collection: &T::CollectionId, item: &T::ItemId, key: &[u8]) -> Option<Vec<u8>> {
        if key.is_empty() {
            // We make the empty key map to the item metadata value.
            ItemMetadataOf::<T>::get(collection, item).map(|m| m.data.into())
        } else {
            // TODO: Len check:
            // let key = BoundedSlice::<_, _>::try_from(key).ok()?;
            Attribute::<T>::get((collection, Some(item), key)).map(|a| a.0.into())
        }
    }

    /// Returns the attribute value of `item` of `collection` corresponding to `key`.
    ///
    /// When `key` is empty, we return the item metadata value.
    ///
    /// By default this is `None`; no attributes are defined.
    fn collection_attribute(collection: &T::CollectionId, key: &[u8]) -> Option<Vec<u8>> {
        if key.is_empty() {
            // We make the empty key map to the item metadata value.
            CollectionMetadataOf::<T>::get(collection).map(|m| m.data.into())
        } else {
            // TODO: Len check:
            // let key = BoundedSlice::<_, _>::try_from(key).ok()?;
            Attribute::<T>::get((collection, Option::<T::ItemId>::None, key)).map(|a| a.0.into())
        }
    }

    /// Returns `true` if the `item` of `collection` may be transferred.
    ///
    /// Default implementation is that all items are transferable.
    fn can_transfer(collection: &T::CollectionId, item: &T::ItemId) -> bool {
        match (Collection::<T>::get(collection), Item::<T>::get(collection, item)) {
            (Some(cd), Some(id)) if !cd.is_frozen && !id.is_frozen => true,
            _ => false,
        }
    }
}

// impl Create
impl<T: Config> Pallet<T> {
    /// Create a `collection` of nonfungible items to be owned by `who` and managed by `admin`.
    fn create_collection(
        collection: &T::CollectionId,
        who: &T::AccountId,
        admin: &T::AccountId,
    ) -> DispatchResultWithPostInfo {
        Self::do_create_collection(
            *collection,
            who.clone(),
            admin.clone(),
            T::CollectionDeposit::get(),
            false,
            Event::Created { collection: *collection, creator: who.clone(), owner: admin.clone() },
        )
    }
}

// impl Destroy
impl<T: Config> Pallet<T> {
    fn get_destroy_witness(collection: &T::CollectionId) -> Option<DestroyWitness> {
        Collection::<T>::get(collection).map(|a| a.destroy_witness())
    }
}

// impl Mutate
impl<T: Config> Pallet<T> {
    fn mint_into(
        collection: &T::CollectionId,
        item: &T::ItemId,
        who: &T::AccountId,
    ) -> DispatchResultWithPostInfo {
        Self::do_mint(*collection, *item, who.clone(), |_| Ok(().into()))
    }
}
