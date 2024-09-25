module example::examplenft {
    use std::string;
    use iota::event;

    /// An example NFT that can be minted by anybody
    public struct ExampleNFT has key, store {
        id: UID,
        /// Name for the token
        name: string::String,
    }

    // ===== Events =====

    public struct NFTMinted has copy, drop {
        // The Object ID of the NFT
        object_id: ID,
        // The creator of the NFT
        creator: address,
        // The name of the NFT
        name: string::String,
    }

    // ===== Public view functions =====

    /// Get the NFT's `name`
    public fun name(nft: &ExampleNFT): &string::String {
        &nft.name
    }

    // ===== Entrypoints =====

    #[allow(lint(self_transfer))]
	/// Anyone can mint a new NFT on this one
    public fun mint_to_sender(
        name: vector<u8>,
        ctx: &mut TxContext
    ) {
        let sender = tx_context::sender(ctx);
        let nft = ExampleNFT {
            id: object::new(ctx),
            name: string::utf8(name)
        };

        event::emit(NFTMinted {
            object_id: object::id(&nft),
            creator: sender,
            name: nft.name,
        });

        transfer::public_transfer(nft, sender);
    }

    /// Transfer `nft` to `recipient`
    public fun transfer(
        nft: ExampleNFT, recipient: address, _: &mut TxContext
    ) {
        transfer::public_transfer(nft, recipient)
    }

    /// Permanently delete `nft`
    public fun burn(nft: ExampleNFT, _: &mut TxContext) {
        let ExampleNFT { id, name: _} = nft;
        object::delete(id)
    }
}