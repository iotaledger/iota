// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { bech32 } from 'bech32';

const IOTA_PREFIX = 'iota';
const HEX_BASE = 16;
const BYTES_PER_CHAR = 2;

// export function toBech32(address: string): string {
//     // Convert the hexadecimal address to a Uint8Array (byte array)
//     const addressBytes = new Uint8Array(address.length / BYTES_PER_CHAR);
//     for (let i = 0; i < address.length; i += BYTES_PER_CHAR) {
//         // Take each pair of hex characters and convert them to a byte
//         addressBytes[i / BYTES_PER_CHAR] = parseInt(address.slice(i, i + BYTES_PER_CHAR), HEX_BASE);
//     }

//     // Convert the byte array to Bech32 words
//     const words = bech32.toWords(addressBytes);

//     // Encode the address using Bech32 with the "iota" prefix
//     const bech32Address = bech32.encode(IOTA_PREFIX, words);
//     return bech32Address;
// }

/**
 * Address type variants.
 */
export enum AddressType {
    /** An Ed25519 address. */
    Ed25519 = 0,
    /** An Alias address. */
    Alias = 8,
    /** An NFT address. */
    Nft = 16,
}

export function toBech32(
    address: string,
    addressType: AddressType = AddressType.Ed25519,
    prefix: string = IOTA_PREFIX,
): string {
    // Convert the hexadecimal address to a Uint8Array (byte array)
    const addressBytes = new Uint8Array(address.length / BYTES_PER_CHAR);
    for (let i = 0; i < address.length; i += BYTES_PER_CHAR) {
        addressBytes[i / BYTES_PER_CHAR] = parseInt(address.slice(i, i + BYTES_PER_CHAR), HEX_BASE);
    }

    // Create a new Uint8Array to hold the address type and address bytes
    const addressWithType = new Uint8Array(1 + addressBytes.length);
    addressWithType[0] = addressType; // First byte: address type
    addressWithType.set(addressBytes, 1); // Remaining bytes: address

    // Convert the address with type to Bech32 words
    const words = bech32.toWords(addressWithType);

    // Encode the address using Bech32 with the specified prefix
    const bech32Address = bech32.encode(prefix, words);
    return bech32Address;
}
