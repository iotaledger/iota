// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[allow(deprecated_usage)]
module stardust::irc27 {

    use std::fixed_point32::FixedPoint32;
    use std::string::String;

    use iota::url::Url;
    use iota::vec_map::VecMap;

    /// The IRC27 NFT metadata standard schema.
    public struct Irc27Metadata has store {
        /// Version of the metadata standard.
        version: String,

        /// The media type (MIME) of the asset.
        ///
        /// ## Examples
        /// - Image files: `image/jpeg`, `image/png`, `image/gif`, etc.
        /// - Video files: `video/x-msvideo` (avi), `video/mp4`, `video/mpeg`, etc.
        /// - Audio files: `audio/mpeg`, `audio/wav`, etc.
        /// - 3D Assets: `model/obj`, `model/u3d`, etc.
        /// - Documents: `application/pdf`, `text/plain`, etc.
        media_type: String,

        /// URL pointing to the NFT file location.
        uri: Url,

        /// Alphanumeric text string defining the human identifiable name for the NFT.
        name: String,

        /// The human-readable collection name of the NFT.
        collection_name: Option<String>,

        /// Royalty payment addresses mapped to the payout percentage.
        /// Contains a hash of the 32 bytes parsed from the BECH32 encoded IOTA address in the metadata, it is a legacy address.
        /// Royalties are not supported by the protocol and needed to be processed by an integrator.
        royalties: VecMap<address, FixedPoint32>,

        /// The human-readable name of the NFT creator.
        issuer_name: Option<String>,

        /// The human-readable description of the NFT.
        description: Option<String>,

        /// Additional attributes which follow [OpenSea Metadata standards](https://docs.opensea.io/docs/metadata-standards).
        attributes: VecMap<String, String>,

        /// Legacy non-standard metadata fields.
        non_standard_fields: VecMap<String, String>,
    }

    /// Get the metadata's `version`.
    public fun version(irc27: &Irc27Metadata): &String {
        &irc27.version
    }

    /// Get the metadata's `media_type`.
    public fun media_type(irc27: &Irc27Metadata): &String {
        &irc27.media_type
    }

    /// Get the metadata's `uri`.
    public fun uri(irc27: &Irc27Metadata): &Url {
        &irc27.uri
    }

    /// Get the metadata's `name`.
    public fun name(irc27: &Irc27Metadata): &String {
        &irc27.name
    }

    /// Get the metadata's `collection_name`.
    public fun collection_name(irc27: &Irc27Metadata): &Option<String> {
        &irc27.collection_name
    }

    /// Get the metadata's `royalties`.
    public fun royalties(irc27: &Irc27Metadata): &VecMap<address, FixedPoint32> {
        &irc27.royalties
    }

    /// Get the metadata's `issuer_name`.
    public fun issuer_name(irc27: &Irc27Metadata): &Option<String> {
        &irc27.issuer_name
    }

    /// Get the metadata's `description`.
    public fun description(irc27: &Irc27Metadata): &Option<String> {
        &irc27.description
    }

    /// Get the metadata's `attributes`.
    public fun attributes(irc27: &Irc27Metadata): &VecMap<String, String> {
        &irc27.attributes
    }

    /// Get the metadata's `non_standard_fields`.
    public fun non_standard_fields(irc27: &Irc27Metadata): &VecMap<String, String> {
        &irc27.non_standard_fields
    }

    /// Permanently destroy a `Irc27Metadata` object.
    public fun destroy(irc27: Irc27Metadata) {
        let Irc27Metadata {
            version: _,
            media_type: _,
            uri: _,
            name: _,
            collection_name: _,
            royalties: _,
            issuer_name: _,
            description: _,
            attributes: _,
            non_standard_fields: _,
        } = irc27;
    }

    #[test_only]
    public fun create_for_testing(
        version: String,
        media_type: String,
        uri: Url,
        name: String,
        collection_name: Option<String>,
        royalties: VecMap<address, FixedPoint32>,
        issuer_name: Option<String>,
        description: Option<String>,
        attributes: VecMap<String, String>,
        non_standard_fields: VecMap<String, String>,
    ): Irc27Metadata {
        Irc27Metadata {
            version,
            media_type,
            uri,
            name,
            collection_name,
            royalties,
            issuer_name,
            description,
            attributes,
            non_standard_fields
        }
    }
}
