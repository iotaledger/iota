// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module custom_nft::collection {
    use std::string::String;

    use iota::event;

    use stardust::alias::Alias;

    // ===== Errors =====

    /// For when someone tries to drop a `Collection` with a wrong capability.
    const EWrongCollectionControllerCap: u64 = 0;

    // ===== Structures =====

    /// A capability allowing the bearer to create or drop an NFT collection.
    /// A `stardust::alias::Alias` instance can be converted into `CollectionControllerCap` in this example,
    /// since an alias address could be used as a collections controller in Stardust.
    /// 
    /// NOTE: To simplify the example, `CollectionControllerCap` is publicly transferable, but to make sure that it can be created,
    /// dropped and owned only by the related `stardust::alias::Alias` owner, we can remove the `store` ability and transfer a created
    /// capability to the sender in the constructor.
    public struct CollectionControllerCap has key, store {
        id: UID,
    }

    /// An NFT collection.
    /// Can be created by a `CollectionControllerCap` owner and used to mint collection-related NFTs.
    /// Can be dropped only by it's `CollectionControllerCap` owner. Once a collection is dropped,
    /// it is impossible to mint new collection-related NFTs.
    ///
    /// NOTE: To simplify the example, `Collection` is publicly transferable, but to make sure that it can be created,
    /// dropped and owned only by the related `CollectionControllerCap` owner, we can remove the `store` ability and transfer a created
    /// capability to the sender in the constructor.
    public struct Collection has key, store {
        id: UID,
        /// The related `CollectionControllerCap` ID.
        cap_id: ID,
        /// The collection name.
        name: String,
    }

    // ===== Events =====

    /// Event marking when a `stardust::alias::Alias` has been converted into `CollectionControllerCap`.
    public struct StardustAliasConverted has copy, drop {
        /// The `stardust::alias::Alias` ID.
        alias_id: ID,
        /// The `CollectionControllerCap` ID.
        cap_id: ID,
    }

    /// Event marking when a `CollectionControllerCap` has been dropped.
    public struct CollectionControllerCapDropped has copy, drop {
        /// The `CollectionControllerCap` ID.
        cap_id: ID,
    }

    /// Event marking when a `Collection` has been created.
    public struct CollectionCreated has copy, drop {
        /// The collection ID.
        collection_id: ID,
    }

    /// Event marking when a `Collection` has been dropped.
    public struct CollectionDropped has copy, drop {
        /// The collection ID.
        collection_id: ID,
    }

    // ===== Public view functions =====

    /// Get the Collection's `name`
    public fun name(nft: &Collection): &String {
        &nft.name
    }

    // ===== Entrypoints =====

    /// Convert a `stardust::alias::Alias` into `CollectionControllerCap`.
    public fun convert_alias_to_collection_controller_cap(stardust_alias: Alias, ctx: &mut TxContext): CollectionControllerCap {
        let cap = CollectionControllerCap {
            id: object::new(ctx)
        };

        event::emit(StardustAliasConverted {
            alias_id: object::id(&stardust_alias),
            cap_id: object::id(&cap),
        });

        stardust::alias::destroy(stardust_alias);

        cap
    }

    /// Drop a `CollectionControllerCap` instance.
    public fun drop_collection_controller_cap(cap: CollectionControllerCap) {
        event::emit(CollectionControllerCapDropped {
            cap_id: object::id(&cap),
        });

        let CollectionControllerCap { id } = cap;

        object::delete(id)
    }

    /// Create a `Collection` instance.
    public fun create_collection(cap: &CollectionControllerCap, name: String, ctx: &mut TxContext): Collection {
        let collection = Collection {
            id: object::new(ctx),
            cap_id: object::id(cap),
            name,
        };

        event::emit(CollectionCreated {
            collection_id: object::id(&collection),
        });

        collection
    }

    /// Drop a `Collection` instance.
    public fun drop_collection(cap: &CollectionControllerCap, collection: Collection) {
        assert!(object::borrow_id(cap) == &collection.cap_id, EWrongCollectionControllerCap);

        event::emit(CollectionDropped {
            collection_id: object::id(&collection),
        });

        let Collection {
            id,
            cap_id: _,
            name: _
        } = collection;

        object::delete(id)
    }
}
