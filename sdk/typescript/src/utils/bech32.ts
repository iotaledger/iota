// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { bech32 } from 'bech32';

const IOTA_PREFIX = 'iota';
const HEX_BASE = 16;
const BYTES_PER_CHAR = 2;

export function toBech32(address: string): string {
    const addressBytes = new Uint8Array(address.length / BYTES_PER_CHAR);
    for (let i = 0; i < address.length; i += BYTES_PER_CHAR) {
        addressBytes[i / BYTES_PER_CHAR] = parseInt(address.slice(i, i + BYTES_PER_CHAR), HEX_BASE);
    }

    const words = bech32.toWords(addressBytes);

    // Encode to Bech32
    const bech32Address = bech32.encode(IOTA_PREFIX, words);
    return bech32Address;
}
