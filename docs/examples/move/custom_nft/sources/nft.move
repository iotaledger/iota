// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module custom_nft::nft {
    use std::string::String;

    use iota::event;
    use iota::url::Url;

    use custom_nft::collection::Collection;

    /// An example NFT that can be minted by anybody.
    public struct Nft has key, store {
        id: UID,
        /// The token name.
        name: String,
        /// The token description.
        description: Option<String>,
        /// The token URL.
        url: Url,
        /// The related collection name.
        collection_name: Option<String>

        // Allow custom attributes.
    }

    // ===== Events =====

    /// Event marking when an `Nft` has been minted.
    public struct NftMinted has copy, drop {
        /// The NFT id.
        object_id: ID,
        /// The NFT creator.
        creator: address,
        /// The NFT name.
        name: String,
    }

    // ===== Public view functions =====

    /// Get the NFT's `name`.
    public fun name(nft: &Nft): &String {
        &nft.name
    }

    /// Get the NFT's `description`.
    public fun description(nft: &Nft): &Option<String> {
        &nft.description
    }

    /// Get the NFT's `url`.
    public fun url(nft: &Nft): &Url {
        &nft.url
    }

    // ===== Entrypoints =====

    /// Convert a `stardust::nft::Nft` into `Nft`.
    /// 
    /// The developer of the `custom_nft` package could tie minting to several conditions, for example:
    /// - Only accept Stardust NFTs from a certain issuer, with a certain name/collection name, `NftId` even.
    /// - Only the `immutable_issuer` and `id` fields count as proof for an NFT belonging to the original collection. 
    /// 
    /// The developer could technically mint the same NFT on the running Stardust network before the mainnet switch
    /// and fake the name and metadata.
    public fun convert(stardust_nft: stardust::nft::Nft, ctx: &mut TxContext): Nft {
        let nft_metadata = stardust_nft.immutable_metadata();

        let nft = mint(
            *nft_metadata.name(),
            *nft_metadata.description(),
            *nft_metadata.uri(),
            *nft_metadata.collection_name(),
            ctx
        );

        stardust::nft::destroy(stardust_nft);

        nft
    }

    /// Mint a collection-related NFT.
    public fun mint_collection_related(
        collection: &Collection,
        name: String,
        description: String,
        url: Url,
        ctx: &mut TxContext
    ): Nft {
        mint(
            name,
            option::some(description),
            url,
            option::some(*collection.name()),
            ctx
        )
    }

    /// Create a new `Nft` instance.
    fun mint(
        name: String,
        description: Option<String>,
        url: Url,
        collection_name: Option<String>,
        ctx: &mut TxContext
    ): Nft {
        let nft = Nft {
            id: object::new(ctx),
            name,
            description,
            url,
            collection_name
        };

        event::emit(NftMinted {
            object_id: object::id(&nft),
            creator: ctx.sender(),
            name: nft.name,
        });

        nft
    }

    /// Permanently delete the `Nft` instance.
    public fun burn(nft: Nft) {
        let Nft {
            id,
            name: _,
            description: _,
            url: _,
            collection_name: _
        } = nft;

        object::delete(id)
    }
}
