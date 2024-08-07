module custom_nft::collection {
    use std::string::String;

    use iota::event;

    use stardust::alias::Alias;

    // ===== Errors =====

    /// For when someone tries to drop a `Collection` with a wrong capability.
    const EWrongCollectionCreatorCap: u64 = 0;

    // ===== Structures =====

    /// A capability allowing the bearer to create an NFT collection.
    /// A `stardust::alias::Alias` instance can be converted into `CollectionCreatorCap`.
    public struct CollectionCreatorCap has key {
        id: UID,
    }

    /// An NFT collection.
    /// Can be used to mint a collection-related NFT.
    public struct Collection has key {
        id: UID,

        /// The related `CollectionCreatorCap` ID.
        cap_id: ID,

        /// The collection name.
        name: Option<String>,
    }

    // ===== Events =====

    /// Event marking when a `stardust::alias::Alias` has been converted into `CollectionCreatorCap`.
    public struct StardustAliasConverted has copy, drop {
        // The `CollectionCreatorCap` ID.
        cap_id: ID,
    }

    /// Event marking when a `CollectionCreatorCap` has been dropped.
    public struct CollectionCreatorCapDropped has copy, drop {
        // The `CollectionCreatorCap` ID.
        cap_id: ID,
    }

    /// Event marking when a `Collection` has been created.
    public struct CollectionCreated has copy, drop {
        // The collection ID.
        collection_id: ID,
    }

    /// Event marking when a `Collection` has been dropped.
    public struct CollectionDropped has copy, drop {
        // The collection ID.
        collection_id: ID,
    }

    // ===== Public view functions =====

    /// Get the Collection's `name`
    public fun name(nft: &Collection): &Option<String> {
        &nft.name
    }

    // ===== Entrypoints =====

    /// Converts a `stardust::alias::Alias` into `CollectionCreatorCap`.
    public fun convert_alias_to_collection_creator_cap(stardust_alias: Alias, ctx: &mut TxContext): CollectionCreatorCap {
        let cap = CollectionCreatorCap {
            id: object::new(ctx)
        };

        stardust::alias::destroy(stardust_alias);

        event::emit(StardustAliasConverted {
            cap_id: object::id(&cap),
        });

        cap
    }

    /// Drops a `CollectionCreatorCap` instance.
    public fun drop_collection_creator_cap(cap: CollectionCreatorCap) {
        event::emit(CollectionCreatorCapDropped {
            cap_id: object::id(&cap),
        });

        let CollectionCreatorCap { id } = cap;

        object::delete(id)
    }

    /// Creates a `Collection` instance.
    public fun create_collection(cap: &CollectionCreatorCap, name: Option<String>, ctx: &mut TxContext): Collection {
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

    /// Drops a `Collection` instance.
    /// Once a collection is dropped, it is impossible to mint collection-related NFTs.
    public fun drop_collection(cap: &CollectionCreatorCap, collection: Collection) {
        assert!(object::borrow_id(cap) == &collection.cap_id, EWrongCollectionCreatorCap);

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
