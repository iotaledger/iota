// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export function objectToKey(obj: object): string {
    const str = JSON.stringify(obj, Object.keys(obj).sort());

    let hash = 0;
    for (let i = 0; i < str.length; i++) {
        const char = str.charCodeAt(i);
        hash = (hash << 5) - hash + char;
        hash |= 0;
    }

    let hashHex = Math.abs(hash).toString(16);
    while (hashHex.length < 8) {
        hashHex = '0' + hashHex;
    }
    return hashHex;
}
