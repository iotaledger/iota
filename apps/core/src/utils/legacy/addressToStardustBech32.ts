// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { bech32 } from 'bech32';
import { hexToBytes } from '@noble/hashes/utils';

const STARDUST_MAINNET_HRP = 'iota';

export enum StardustAddressType {
    Ed25519 = 0,
    Alias = 8,
    Nft = 16,
}

export function addressToStardustBech32(
    address: string,
    addressType: StardustAddressType = StardustAddressType.Ed25519,
    hrp: string = STARDUST_MAINNET_HRP,
): string {
    // We first need to check if the address is a valid hex string
    const originalAddressHexValue = address.startsWith('0x') ? address.slice(2) : address;
    // Convert the original hex address to bytes
    const originalAddressBytes = hexToBytes(originalAddressHexValue);
    // Create a new Uint8Array with the length of the original hex address + 1 to accommodate the address type
    const stardustAddressDataArray = new Uint8Array(1 + originalAddressBytes.length);
    // Set the first byte to the address type
    stardustAddressDataArray[0] = addressType;
    // Copy the original address bytes into the new array starting from index 1
    stardustAddressDataArray.set(originalAddressBytes, 1);
    // Encode to Bech32 format
    return bech32.encode(hrp, bech32.toWords(stardustAddressDataArray));
}
