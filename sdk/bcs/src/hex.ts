// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export function fromHex(hexStr: string): Uint8Array {
    const normalized = hexStr.startsWith('0x') ? hexStr.slice(2) : hexStr;
    const padded = normalized.length % 2 === 0 ? normalized : `0${normalized}`;
    const intArr = padded.match(/[0-9a-fA-F]{2}/g)?.map((byte) => parseInt(byte, 16)) ?? [];

    if (intArr.length !== padded.length / 2) {
        throw new Error(`Invalid hex string ${hexStr}`);
    }

    return Uint8Array.from(intArr);
}

export function toHex(bytes: Uint8Array): string {
    return bytes.reduce((str, byte) => str + byte.toString(16).padStart(2, '0'), '');
}
