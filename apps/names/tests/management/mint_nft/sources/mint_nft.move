module mint_nft::mint_nft {
    use std::string::{Self, String};
    use iota::tx_context::{sender};

    // import the package and display modules for the NFT display
    use iota::package;
    use iota::display;

    public struct Nft has key, store {
        id: UID,
        name: String,
        image_url: String,
        description: String,
    }

    // has to be the same name as the module all uppercased
    public struct MINT_NFT has drop {}

    fun init(otw: MINT_NFT, ctx: &mut TxContext) {
        let keys = vector[
            string::utf8(b"name"),
            string::utf8(b"image_url"),
            string::utf8(b"description"),
        ];

        let values = vector[
            string::utf8(b"{name}"),
            string::utf8(b"{image_url}"),
            string::utf8(b"{description}")
        ];

        // Claim the publisher
        let publisher = package::claim(otw, ctx);

        // Get a new `Display` object for the `Nft` type.
        let mut display = display::new_with_fields<Nft>(&publisher, keys, values, ctx);
        display::update_version(&mut display);

        transfer::public_transfer(publisher, sender(ctx));
        transfer::public_transfer(display, sender(ctx));
    }

    public entry fun mint(name: vector<u8>, description: vector<u8>, image_url: vector<u8>, ctx: &mut TxContext) {
        let nft = Nft {
            id: object::new(ctx),
            name: string::utf8(name),
            image_url: string::utf8(image_url),
            description: string::utf8(description),
        };

        // transfer the Nft to the caller
        let sender = sender(ctx);
        transfer::public_transfer(nft, sender);
    }

    /// Permanently delete `nft`
    public fun burn(nft: Nft, _: &mut TxContext) {
        let Nft {
            id,
            name: _,
            image_url: _,
            description: _,
        } = nft;


        object::delete(id)
    }
}
