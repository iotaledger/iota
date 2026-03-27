// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Helper utility: write number as an ULEB array.
// Original code is taken from: https://www.npmjs.com/package/uleb128 (no longer exists)
export function ulebEncode(num: number | bigint): number[] {
    let bigNum = BigInt(num);
    const arr: number[] = [];
    let len = 0;

    if (bigNum === 0n) {
        return [0];
    }

    while (bigNum > 0) {
        arr[len] = Number(bigNum & 0x7fn);
        bigNum >>= 7n;
        if (bigNum > 0n) {
            arr[len] |= 0x80;
        }
        len += 1;
    }

    return arr;
}

// Helper utility: decode ULEB as an array of numbers.
// Original code is taken from: https://www.npmjs.com/package/uleb128 (no longer exists)
export function ulebDecode(arr: number[] | Uint8Array): {
    value: number;
    length: number;
} {
    let total = 0n;
    let shift = 0n;
    let len = 0;

    // eslint-disable-next-line no-constant-condition
    while (true) {
        if (len >= arr.length) {
            throw new Error('ULEB decode error: buffer overflow');
        }

        const byte = arr[len];
        len += 1;
        total += BigInt(byte & 0x7f) << shift;
        if ((byte & 0x80) === 0) {
            break;
        }
        shift += 7n;
    }

    // TODO: return bigint in next major version
    if (total > BigInt(Number.MAX_SAFE_INTEGER)) {
        throw new Error('ULEB decode error: value exceeds MAX_SAFE_INTEGER');
    }

    return {
        value: Number(total),
        length: len,
    };
}
