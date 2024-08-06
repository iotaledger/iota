module random_nft::random_nft {
    use iota::url::Url;
    use stardust::nft::Nft;
    use std::string::String;
    use iota::event;

    /// An example NFT that can be minted by anybody
    public struct RandomNFT has key, store {
        id: UID,
        /// Name for the token
        name: String,
        /// Description of the token
        description: Option<String>,
        /// URL for the token
        url: Url,

        collection_name: Option<String>
        // TODO: allow custom attributes
    }

    // ===== Events =====

    public struct RandomNFTMinted has copy, drop {
        // The Object ID of the NFT
        object_id: ID,
        // The creator of the NFT
        creator: address,
        // The name of the NFT
        name: String,
    }

    // ===== Public view functions =====

    /// Get the NFT's `name`
    public fun name(nft: &RandomNFT): &String {
        &nft.name
    }

    /// Get the NFT's `description`
    public fun description(nft: &RandomNFT): &Option<String> {
        &nft.description
    }

    /// Get the NFT's `url`
    public fun url(nft: &RandomNFT): &Url {
        &nft.url
    }

    // ===== Entrypoints =====
    
    public fun convert(stardust_nft: Nft, ctx: &mut TxContext) {
        let nft_metadata = stardust_nft.immutable_metadata();

        mint(*nft_metadata.name(), *nft_metadata.description(), *nft_metadata.uri(), *nft_metadata.collection_name(), ctx);

        stardust::nft::destroy(stardust_nft)
    }

    #[allow(lint(self_transfer))]
    /// Create a new RandomNFT
    public fun mint(
        name: String,
        description: Option<String>,
        url: Url,
        collection_name: Option<String>,
        ctx: &mut TxContext
    ) {
        let sender = ctx.sender();
        let nft = RandomNFT {
            id: object::new(ctx),
            name,
            description,
            url,
            collection_name
        };

        event::emit(RandomNFTMinted {
            object_id: object::id(&nft),
            creator: sender,
            name: nft.name,
        });

        transfer::public_transfer(nft, sender);
    }

    /// Permanently delete `nft`
    public fun burn(nft: RandomNFT, _: &mut TxContext) {
        let RandomNFT { id, name: _, description: _, url: _ , collection_name: _ } = nft;
        id.delete()
    }
}