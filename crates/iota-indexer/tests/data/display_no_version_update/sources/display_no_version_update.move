// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Test module that creates a `Display<Nft>` on publish without ever calling
/// `display::update_version`, so the publish transaction emits only a
/// `DisplayCreated` event and the display fields exist solely in the
/// `Display` object.
module display_no_version_update::display_no_version_update {
    use std::string::String;
    use iota::display;
    use iota::package;

    public struct DISPLAY_NO_VERSION_UPDATE has drop {}

    public struct Nft has key, store {
        id: UID,
        name: String,
    }

    fun init(otw: DISPLAY_NO_VERSION_UPDATE, ctx: &mut TxContext) {
        let publisher = package::claim(otw, ctx);
        let display = display::new_with_fields<Nft>(
            &publisher,
            vector[b"name".to_string(), b"description".to_string()],
            vector[b"{name}".to_string(), b"An NFT with display".to_string()],
            ctx,
        );
        transfer::public_transfer(display, ctx.sender());
        transfer::public_transfer(publisher, ctx.sender());
    }

    public entry fun mint(name: String, ctx: &mut TxContext) {
        transfer::public_transfer(Nft { id: object::new(ctx), name }, ctx.sender());
    }
}
