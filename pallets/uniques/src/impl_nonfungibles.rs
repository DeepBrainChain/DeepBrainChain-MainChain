// This file is part of Substrate.

// Copyright (C) 2017-2022 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Implementations for `nonfungibles` traits.

use super::*;
use frame_support::{
    storage::KeyPrefixIterator,
    traits::{tokens::nonfungibles::*, Get},
    BoundedSlice,
};
use sp_runtime::{DispatchError, DispatchResult};
use sp_std::prelude::*;

impl<T: Config> Inspect<<T as SystemConfig>::AccountId> for Pallet<T> {
    type ItemId = T::ItemId;
    type CollectionId = T::CollectionId;

    fn owner(
        collection: &Self::CollectionId,
        item: &Self::ItemId,
    ) -> Option<<T as SystemConfig>::AccountId> {
        Item::<T>::get(collection, item).map(|a| a.owner)
    }

    fn collection_owner(collection: &Self::CollectionId) -> Option<<T as SystemConfig>::AccountId> {
        Collection::<T>::get(collection).map(|a| a.owner)
    }

    /// Returns the attribute value of `item` of `collection` corresponding to `key`.
    ///
    /// When `key` is empty, we return the item metadata value.
    ///
    /// By default this is `None`; no attributes are defined.
    fn attribute(
        collection: &Self::CollectionId,
        item: &Self::ItemId,
        key: &[u8],
    ) -> Option<Vec<u8>> {
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
    fn collection_attribute(collection: &Self::CollectionId, key: &[u8]) -> Option<Vec<u8>> {
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
    fn can_transfer(collection: &Self::CollectionId, item: &Self::ItemId) -> bool {
        match (Collection::<T>::get(collection), Item::<T>::get(collection, item)) {
            (Some(cd), Some(id)) if !cd.is_frozen && !id.is_frozen => true,
            _ => false,
        }
    }
}

impl<T: Config> Create<<T as SystemConfig>::AccountId> for Pallet<T> {
    /// Create a `collection` of nonfungible items to be owned by `who` and managed by `admin`.
    fn create_collection(
        collection: &Self::CollectionId,
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

impl<T: Config> Destroy<<T as SystemConfig>::AccountId> for Pallet<T> {
    type DestroyWitness = DestroyWitness;

    fn get_destroy_witness(collection: &Self::CollectionId) -> Option<DestroyWitness> {
        Collection::<T>::get(collection).map(|a| a.destroy_witness())
    }

    fn destroy(
        collection: Self::CollectionId,
        witness: Self::DestroyWitness,
        maybe_check_owner: Option<T::AccountId>,
    ) -> Result<Self::DestroyWitness, DispatchError> {
        Self::do_destroy_collection(collection, witness, maybe_check_owner)
    }
}

impl<T: Config> Mutate<<T as SystemConfig>::AccountId> for Pallet<T> {
    fn mint_into(
        collection: &Self::CollectionId,
        item: &Self::ItemId,
        who: &T::AccountId,
    ) -> DispatchResult {
        Self::do_mint(*collection, *item, who.clone(), |_| Ok(()))
    }

    fn burn(
        collection: &Self::CollectionId,
        item: &Self::ItemId,
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

impl<T: Config> Transfer<T::AccountId> for Pallet<T> {
    fn transfer(
        collection: &Self::CollectionId,
        item: &Self::ItemId,
        destination: &T::AccountId,
    ) -> DispatchResult {
        Self::do_transfer(*collection, *item, destination.clone(), |_, _| Ok(()))
    }
}

impl<T: Config> InspectEnumerable<T::AccountId> for Pallet<T> {
    type CollectionsIterator = KeyPrefixIterator<<T as Config>::CollectionId>;
    type ItemsIterator = KeyPrefixIterator<<T as Config>::ItemId>;
    type OwnedIterator = KeyPrefixIterator<(<T as Config>::CollectionId, <T as Config>::ItemId)>;
    type OwnedInCollectionIterator = KeyPrefixIterator<<T as Config>::ItemId>;

    /// Returns an iterator of the collections in existence.
    ///
    /// NOTE: iterating this list invokes a storage read per item.
    fn collections() -> Self::CollectionsIterator {
        CollectionMetadataOf::<T>::iter_keys()
    }

    /// Returns an iterator of the items of a `collection` in existence.
    ///
    /// NOTE: iterating this list invokes a storage read per item.
    fn items(collection: &Self::CollectionId) -> Self::ItemsIterator {
        ItemMetadataOf::<T>::iter_key_prefix(collection)
    }

    /// Returns an iterator of the items of all collections owned by `who`.
    ///
    /// NOTE: iterating this list invokes a storage read per item.
    fn owned(who: &T::AccountId) -> Self::OwnedIterator {
        Account::<T>::iter_key_prefix((who,))
    }

    /// Returns an iterator of the items of `collection` owned by `who`.
    ///
    /// NOTE: iterating this list invokes a storage read per item.
    fn owned_in_collection(
        collection: &Self::CollectionId,
        who: &T::AccountId,
    ) -> Self::OwnedInCollectionIterator {
        Account::<T>::iter_key_prefix((who, collection))
    }
}
