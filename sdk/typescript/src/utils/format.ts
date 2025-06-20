// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isValidIotaAddress, isValidIotaObjectId, IOTA_ADDRESS_LENGTH } from './iota-types.js';

const ELLIPSIS = '\u{2026}';

export function formatAddress(address: string) {
    if (address.length <= 6) {
        return address;
    }

    const offset = address.startsWith('0x') ? 2 : 0;

    return `0x${address.slice(offset, offset + 4)}${ELLIPSIS}${address.slice(-4)}`;
}

export function formatDigest(digest: string) {
    // Use 10 first characters
    return `${digest.slice(0, 10)}${ELLIPSIS}`;
}

export function formatType(type: string) {
    const objectAddressPattern = new RegExp(`0x[a-fA-F0-9]{${IOTA_ADDRESS_LENGTH * 2}}`, 'g');
    const matches = type.match(objectAddressPattern) ?? [];
    for (const match of matches) {
        if (isValidIotaAddress(match) || isValidIotaObjectId(match)) {
            type = type.replace(match, formatAddress(match));
        }
    }
    return type;
}

const ADDRESS_TRIM_MAX_LENGTH = 8;

export function trimAddress(address: string): string {
    const addr = address.toLowerCase().replace(/^0x/, '');
    const shortened = addr.replace(/^0+/, '') || '0';
    return `0x${shortened}`;
}

export function trimOrFormatAddress(address: string): string {
    if (address.length <= 6) {
        return address;
    }

    const trimmedAddress = trimAddress(address);

    if (trimmedAddress.length <= ADDRESS_TRIM_MAX_LENGTH) {
        return trimmedAddress;
    }

    return formatAddress(address);
}
