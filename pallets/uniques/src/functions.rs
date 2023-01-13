//! Various pieces of common functionality.

use super::{Account, *};
use frame_support::{
    ensure,
    pallet_prelude::DispatchResultWithPostInfo,
    traits::{ExistenceRequirement, Get},
    IterableStorageMap,
};
use sp_runtime::{traits::One, DispatchError, DispatchResult};

impl<T: Config> Pallet<T> {
    pub fn do_transfer(
        collection: T::CollectionId,
        item: T::ItemId,
        dest: T::AccountId,
        with_details: impl FnOnce(&CollectionDetailsFor<T>, &mut ItemDetailsFor<T>) -> DispatchResult,
    ) -> DispatchResultWithPostInfo {
        let collection_details =
            Collection::<T>::get(&collection).ok_or(Error::<T>::UnknownCollection)?;
        ensure!(!collection_details.is_frozen, Error::<T>::Frozen);
        // ensure!(!T::Locker::is_locked(collection, item), Error::<T>::Locked);

        let mut details =
            Item::<T>::get(&collection, &item).ok_or(Error::<T>::UnknownCollection)?;
        ensure!(!details.is_frozen, Error::<T>::Frozen);
        with_details(&collection_details, &mut details)?;

        Account::<T>::remove((&details.owner, &collection, &item));
        Account::<T>::insert((&dest, &collection, &item), ());
        let origin = details.owner;
        details.owner = dest;

        // The approved account has to be reset to None, because otherwise pre-approve attack would
        // be possible, where the owner can approve his second account before making the transaction
        // and then claiming the item back.
        details.approved = None;

        Item::<T>::insert(&collection, &item, &details);
        ItemPriceOf::<T>::remove(&collection, &item);

        Self::deposit_event(Event::Transferred {
            collection,
            item,
            from: origin,
            to: details.owner,
        });
        Ok(().into())
    }

    pub fn do_create_collection(
        collection: T::CollectionId,
        owner: T::AccountId,
        admin: T::AccountId,
        deposit: DepositBalanceOf<T>,
        free_holding: bool,
        event: Event<T>,
    ) -> DispatchResultWithPostInfo {
        ensure!(!Collection::<T>::contains_key(collection), Error::<T>::InUse);

        T::Currency::reserve(&owner, deposit)?;

        Collection::<T>::insert(
            collection,
            CollectionDetails {
                owner: owner.clone(),
                issuer: admin.clone(),
                admin: admin.clone(),
                freezer: admin,
                total_deposit: deposit,
                free_holding,
                items: 0,
                item_metadatas: 0,
                attributes: 0,
                is_frozen: false,
            },
        );

        CollectionAccount::<T>::insert(&owner, &collection, ());
        Self::deposit_event(event);
        Ok(().into())
    }

    pub fn do_destroy_collection(
        collection: T::CollectionId,
        witness: DestroyWitness,
        maybe_check_owner: Option<T::AccountId>,
    ) -> Result<DestroyWitness, DispatchError> {
        Collection::<T>::try_mutate_exists(collection, |maybe_details| {
            let collection_details = maybe_details.take().ok_or(Error::<T>::UnknownCollection)?;
            if let Some(check_owner) = maybe_check_owner {
                ensure!(collection_details.owner == check_owner, Error::<T>::NoPermission);
            }
            ensure!(collection_details.items == witness.items, Error::<T>::BadWitness);
            ensure!(
                collection_details.item_metadatas == witness.item_metadatas,
                Error::<T>::BadWitness
            );
            ensure!(collection_details.attributes == witness.attributes, Error::<T>::BadWitness);

            for (item, details) in Item::<T>::drain_prefix(&collection) {
                Account::<T>::remove((&details.owner, &collection, &item));
            }
            #[allow(deprecated)]
            ItemMetadataOf::<T>::remove_prefix(&collection);
            #[allow(deprecated)]
            ItemPriceOf::<T>::remove_prefix(&collection);
            CollectionMetadataOf::<T>::remove(&collection);
            // #[allow(deprecated)]
            // Attribute::<T>::remove_prefix((&collection,), None);
            let all_attribute_key = Self::get_all_attribute_key();
            for a_attribute in all_attribute_key {
                if a_attribute.0 == collection {
                    Attribute::<T>::remove(a_attribute);
                }
            }

            CollectionAccount::<T>::remove(&collection_details.owner, &collection);
            T::Currency::unreserve(&collection_details.owner, collection_details.total_deposit);
            CollectionMaxSupply::<T>::remove(&collection);

            Self::deposit_event(Event::Destroyed { collection });

            Ok(DestroyWitness {
                items: collection_details.items,
                item_metadatas: collection_details.item_metadatas,
                attributes: collection_details.attributes,
            })
        })
    }

    pub fn do_mint(
        collection: T::CollectionId,
        item: T::ItemId,
        owner: T::AccountId,
        with_details: impl FnOnce(&CollectionDetailsFor<T>) -> DispatchResult,
    ) -> DispatchResultWithPostInfo {
        ensure!(!Item::<T>::contains_key(collection, item), Error::<T>::AlreadyExists);

        Collection::<T>::try_mutate(&collection, |maybe_collection_details| -> DispatchResult {
            let collection_details =
                maybe_collection_details.as_mut().ok_or(Error::<T>::UnknownCollection)?;

            with_details(collection_details)?;

            if let Ok(max_supply) = CollectionMaxSupply::<T>::try_get(&collection) {
                ensure!(collection_details.items < max_supply, Error::<T>::MaxSupplyReached);
            }

            let items = collection_details.items.checked_add(1).ok_or(Error::<T>::Overflow)?;
            collection_details.items = items;

            let deposit = match collection_details.free_holding {
                true => Zero::zero(),
                false => T::ItemDeposit::get(),
            };
            T::Currency::reserve(&collection_details.owner, deposit)?;
            collection_details.total_deposit += deposit;

            let owner = owner.clone();
            Account::<T>::insert((&owner, &collection, &item), ());
            let details = ItemDetails { owner, approved: None, is_frozen: false, deposit };
            Item::<T>::insert(&collection, &item, details);
            Ok(())
        })?;

        Self::deposit_event(Event::Issued { collection, item, owner });
        Ok(().into())
    }

    pub fn do_burn(
        collection: T::CollectionId,
        item: T::ItemId,
        with_details: impl FnOnce(&CollectionDetailsFor<T>, &ItemDetailsFor<T>) -> DispatchResult,
    ) -> DispatchResultWithPostInfo {
        // ensure!(!T::Locker::is_locked(collection, item), Error::<T>::Locked);
        let owner = Collection::<T>::try_mutate(
            &collection,
            |maybe_collection_details| -> Result<T::AccountId, DispatchError> {
                let collection_details =
                    maybe_collection_details.as_mut().ok_or(Error::<T>::UnknownCollection)?;
                let details =
                    Item::<T>::get(&collection, &item).ok_or(Error::<T>::UnknownCollection)?;
                with_details(collection_details, &details)?;

                // Return the deposit.
                T::Currency::unreserve(&collection_details.owner, details.deposit);

                collection_details.total_deposit =
                    collection_details.total_deposit.saturating_sub(details.deposit);
                collection_details.items = collection_details.items.saturating_sub(One::one());
                Ok(details.owner)
            },
        )?;

        Item::<T>::remove(&collection, &item);
        Account::<T>::remove((&owner, &collection, &item));
        ItemPriceOf::<T>::remove(&collection, &item);

        Self::deposit_event(Event::Burned { collection, item, owner });
        Ok(().into())
    }

    pub fn do_set_price(
        collection: T::CollectionId,
        item: T::ItemId,
        sender: T::AccountId,
        price: Option<ItemPrice<T>>,
        whitelisted_buyer: Option<T::AccountId>,
    ) -> DispatchResultWithPostInfo {
        let details = Item::<T>::get(&collection, &item).ok_or(Error::<T>::UnknownItem)?;
        ensure!(details.owner == sender, Error::<T>::NoPermission);

        if let Some(ref price) = price {
            ItemPriceOf::<T>::insert(&collection, &item, (price, whitelisted_buyer.clone()));
            Self::deposit_event(Event::ItemPriceSet {
                collection,
                item,
                price: *price,
                whitelisted_buyer,
            });
        } else {
            ItemPriceOf::<T>::remove(&collection, &item);
            Self::deposit_event(Event::ItemPriceRemoved { collection, item });
        }

        Ok(().into())
    }

    pub fn do_buy_item(
        collection: T::CollectionId,
        item: T::ItemId,
        buyer: T::AccountId,
        bid_price: ItemPrice<T>,
    ) -> DispatchResultWithPostInfo {
        let details = Item::<T>::get(&collection, &item).ok_or(Error::<T>::UnknownItem)?;
        ensure!(details.owner != buyer, Error::<T>::NoPermission);

        let price_info = ItemPriceOf::<T>::get(&collection, &item).ok_or(Error::<T>::NotForSale)?;

        ensure!(bid_price >= price_info.0, Error::<T>::BidTooLow);

        if let Some(only_buyer) = price_info.1 {
            ensure!(only_buyer == buyer, Error::<T>::NoPermission);
        }

        T::Currency::transfer(
            &buyer,
            &details.owner,
            price_info.0,
            ExistenceRequirement::KeepAlive,
        )?;

        let old_owner = details.owner.clone();

        Self::do_transfer(collection, item, buyer.clone(), |_, _| Ok(()))?;

        Self::deposit_event(Event::ItemBought {
            collection,
            item,
            price: price_info.0,
            seller: old_owner,
            buyer,
        });

        Ok(().into())
    }

    pub fn get_all_attribute_key() -> Vec<(T::CollectionId, Option<T::ItemId>, Vec<u8>)> {
        <Attribute<T> as IterableStorageMap<(T::CollectionId, Option<T::ItemId>, Vec<u8>), _>>::iter()
            .map(|((collection_id, item_id, num), _)| (collection_id, item_id, num))
            .collect::<Vec<_>>()
    }
}
