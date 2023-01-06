//! Implementations for `nonfungibles` traits.

use super::*;
// use frame_support::{KeyPrefixIterator, BoundedSlice};
// use frame_support::traits::tokens::nonfungibles::*;
use frame_support::traits::Get;
use sp_runtime::{DispatchError, DispatchResult};
use sp_std::prelude::*;

// impl Inspect
impl<T: Config> Pallet<T> {
    fn owner(
        collection: &T::CollectionId,
        item: &T::ItemId,
    ) -> Option<<T as SystemConfig>::AccountId> {
        Item::<T>::get(collection, item).map(|a| a.owner)
    }

    fn collection_owner(collection: &T::CollectionId) -> Option<<T as SystemConfig>::AccountId> {
        Collection::<T>::get(collection).map(|a| a.owner)
    }

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
            let key = BoundedSlice::<_, _>::try_from(key).ok()?;
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
            let key = BoundedSlice::<_, _>::try_from(key).ok()?;
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
    ) -> DispatchResult {
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

    fn destroy(
        collection: T::CollectionId,
        witness: DestroyWitness,
        maybe_check_owner: Option<T::AccountId>,
    ) -> Result<DestroyWitness, DispatchError> {
        Self::do_destroy_collection(collection, witness, maybe_check_owner)
    }
}

// impl Mutate
impl<T: Config> Pallet<T> {
    fn mint_into(
        collection: &T::CollectionId,
        item: &T::ItemId,
        who: &T::AccountId,
    ) -> DispatchResult {
        Self::do_mint(*collection, *item, who.clone(), |_| Ok(()))
    }

    fn burn(
        collection: &T::CollectionId,
        item: &T::ItemId,
        maybe_check_owner: Option<&T::AccountId>,
    ) -> DispatchResult {
        Self::do_burn(*collection, *item, |_, d| {
            if let Some(check_owner) = maybe_check_owner {
                if &d.owner != check_owner {
                    return Err(Error::<T>::NoPermission.into())
                }
            }
            Ok(())
        })
    }
}

// impl Transfer
impl<T: Config> Pallet<T> {
    fn transfer(
        collection: &T::CollectionId,
        item: &T::ItemId,
        destination: &T::AccountId,
    ) -> DispatchResult {
        Self::do_transfer(*collection, *item, destination.clone(), |_, _| Ok(()))
    }
}

// // TODO: impl InspectEnumerable
// impl<T: Config> Pallet<T> {
//     /// Returns an iterator of the collections in existence.
//     ///
//     /// NOTE: iterating this list invokes a storage read per item.
//     fn collections() -> Self::CollectionsIterator {
//         CollectionMetadataOf::<T>::iter_keys()
//     }

//     /// Returns an iterator of the items of a `collection` in existence.
//     ///
//     /// NOTE: iterating this list invokes a storage read per item.
//     fn items(collection: &Self::CollectionId) -> Self::ItemsIterator {
//         ItemMetadataOf::<T>::iter_key_prefix(collection)
//     }

//     /// Returns an iterator of the items of all collections owned by `who`.
//     ///
//     /// NOTE: iterating this list invokes a storage read per item.
//     fn owned(who: &T::AccountId) -> Self::OwnedIterator {
//         Account::<T>::iter_key_prefix((who,))
//     }

//     /// Returns an iterator of the items of `collection` owned by `who`.
//     ///
//     /// NOTE: iterating this list invokes a storage read per item.
//     fn owned_in_collection(
//         collection: &Self::CollectionId,
//         who: &T::AccountId,
//     ) -> Self::OwnedInCollectionIterator {
//         Account::<T>::iter_key_prefix((who, collection))
//     }
// }

// impl<T: Config> InspectEnumerable<T::AccountId> for Pallet<T> {
//     type CollectionsIterator = KeyPrefixIterator<<T as Config>::CollectionId>;
//     type ItemsIterator = KeyPrefixIterator<<T as Config>::ItemId>;
//     type OwnedIterator = KeyPrefixIterator<(<T as Config>::CollectionId, <T as Config>::ItemId)>;
//     type OwnedInCollectionIterator = KeyPrefixIterator<<T as Config>::ItemId>;

//     /// Returns an iterator of the collections in existence.
//     ///
//     /// NOTE: iterating this list invokes a storage read per item.
//     fn collections() -> Self::CollectionsIterator {
//         CollectionMetadataOf::<T>::iter_keys()
//     }

//     /// Returns an iterator of the items of a `collection` in existence.
//     ///
//     /// NOTE: iterating this list invokes a storage read per item.
//     fn items(collection: &Self::CollectionId) -> Self::ItemsIterator {
//         ItemMetadataOf::<T>::iter_key_prefix(collection)
//     }

//     /// Returns an iterator of the items of all collections owned by `who`.
//     ///
//     /// NOTE: iterating this list invokes a storage read per item.
//     fn owned(who: &T::AccountId) -> Self::OwnedIterator {
//         Account::<T>::iter_key_prefix((who,))
//     }

//     /// Returns an iterator of the items of `collection` owned by `who`.
//     ///
//     /// NOTE: iterating this list invokes a storage read per item.
//     fn owned_in_collection(
//         collection: &Self::CollectionId,
//         who: &T::AccountId,
//     ) -> Self::OwnedInCollectionIterator {
//         Account::<T>::iter_key_prefix((who, collection))
//     }
// }
