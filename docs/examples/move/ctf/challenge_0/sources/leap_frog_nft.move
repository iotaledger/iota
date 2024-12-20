module leapFrog::leap_frog_nft {
    use iota::url::{Self, Url};
    use std::string;
    use iota::event;

    public struct LeapFrogNFT has key, store {
        id: UID,
        name: string::String,
        description: string::String,
        url: Url,
    }

    public struct NFTMinted has copy, drop {
        object_id: ID,
        creator: address,
        name: string::String,
    }

    public fun name(nft: &LeapFrogNFT): &string::String {
        &nft.name
    }

    public fun description(nft: &LeapFrogNFT): &string::String {
        &nft.description
    }

    public fun url(nft: &LeapFrogNFT): &Url {
        &nft.url
    }

    public fun mint_to_sender(
        name: vector<u8>,
        description: vector<u8>,
        url: vector<u8>,
        recipient: address,
        ctx: &mut TxContext
    ) {
        let sender = tx_context::sender(ctx);
        let nft = LeapFrogNFT {
            id: object::new(ctx),
            name: string::utf8(name),
            description: string::utf8(description),
            url: url::new_unsafe_from_bytes(url)
        };

        event::emit(NFTMinted {
            object_id: object::id(&nft),
            creator: sender,
            name: nft.name,
        });

        transfer::public_transfer(nft, recipient);
    }

    public fun transfer(
        nft: LeapFrogNFT, recipient: address, _: &mut TxContext
    ) {
        transfer::public_transfer(nft, recipient)
    }

    public fun update_description(
        nft: &mut LeapFrogNFT,
        new_description: vector<u8>,
        _: &mut TxContext
    ) {
        nft.description = string::utf8(new_description)
    }
    
    public fun burn(nft: LeapFrogNFT, _: &mut TxContext) {
        let LeapFrogNFT { id, name: _, description: _, url: _ } = nft;
        object::delete(id)
    }
}