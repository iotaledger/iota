module random_nft::random_nft {
    use iota::url::{Self, Url};
    use std::string;
    use iota::event;

    /// An example NFT that can be minted by anybody
    public struct RandomNFT has key, store {
        id: UID,
        /// Name for the token
        name: string::String,
        /// Description of the token
        description: string::String,
        /// URL for the token
        url: Url,
        // TODO: allow custom attributes
    }

    // ===== Events =====

    public struct RandomNFTMinted has copy, drop {
        // The Object ID of the NFT
        object_id: ID,
        // The creator of the NFT
        creator: address,
        // The name of the NFT
        name: string::String,
    }

    // ===== Public view functions =====

    /// Get the NFT's `name`
    public fun name(nft: &RandomNFT): &string::String {
        &nft.name
    }

    /// Get the NFT's `description`
    public fun description(nft: &RandomNFT): &string::String {
        &nft.description
    }

    /// Get the NFT's `url`
    public fun url(nft: &RandomNFT): &Url {
        &nft.url
    }

    // ===== Entrypoints =====

    #[allow(lint(self_transfer))]
    /// Create a new RandomNFT
    public fun mint(
        name: vector<u8>,
        description: vector<u8>,
        url: vector<u8>,
        receiver: address,
        ctx: &mut TxContext
    ) {
        let sender = ctx.sender();
        let nft = RandomNFT {
            id: object::new(ctx),
            name: string::utf8(name),
            description: string::utf8(description),
            url: url::new_unsafe_from_bytes(url)
        };

        event::emit(RandomNFTMinted {
            object_id: object::id(&nft),
            creator: sender,
            name: nft.name,
        });

        transfer::public_transfer(nft, receiver);
    }

    /// Permanently delete `nft`
    public fun burn(nft: RandomNFT, _: &mut TxContext) {
        let RandomNFT { id, name: _, description: _, url: _ } = nft;
        id.delete()
    }
}