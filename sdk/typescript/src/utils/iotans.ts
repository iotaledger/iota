// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const IOTA_NS_NAME_REGEX =
    /^(?!.*(^(?!@)|[-.@])($|[-.@]))(?:[a-z0-9-]{0,63}(?:\.[a-z0-9-]{0,63})*)?@[a-z0-9-]{0,63}$/i;
const IOTA_NS_DOMAIN_REGEX = /^(?!.*(^|[-.])($|[-.]))(?:[a-z0-9-]{0,63}\.)+iota$/i;
const MAX_IOTA_NS_NAME_LENGTH = 235;

export function isValidIotaNSName(name: string): boolean {
    if (name.length > MAX_IOTA_NS_NAME_LENGTH) {
        return false;
    }

    if (name.includes('@')) {
        return IOTA_NS_NAME_REGEX.test(name);
    }

    return IOTA_NS_DOMAIN_REGEX.test(name);
}

export function normalizeIotaNSName(name: string, format: 'at' | 'dot' = 'at'): string {
    const lowerCase = name.toLowerCase();
    let parts;

    if (lowerCase.includes('@')) {
        if (!IOTA_NS_NAME_REGEX.test(lowerCase)) {
            throw new Error(`Invalid IotaNS name ${name}`);
        }
        const [labels, domain] = lowerCase.split('@');
        parts = [...(labels ? labels.split('.') : []), domain];
    } else {
        if (!IOTA_NS_DOMAIN_REGEX.test(lowerCase)) {
            throw new Error(`Invalid IotaNS name ${name}`);
        }
        parts = lowerCase.split('.').slice(0, -1);
    }

    if (format === 'dot') {
        return `${parts.join('.')}.iota`;
    }

    return `${parts.slice(0, -1).join('.')}@${parts[parts.length - 1]}`;
}
