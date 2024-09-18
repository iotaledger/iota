// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export interface MakeDerivationOptions {
    bip44CoinType?: number;
    accountIndex: number;
    changeIndex?: number;
    addressIndex?: number;
}

export function makeDerivationPath({
    bip44CoinType = 4218,
    accountIndex,
    changeIndex = 0,
    addressIndex = 0,
}: MakeDerivationOptions) {
    // currently returns only Ed25519 path
    return `m/44'/${bip44CoinType}'/${accountIndex}'/${changeIndex}'/${addressIndex}'`;
}

/**
 * Parses a derivation path string and returns its components.
 *
 * @param path - The derivation path string (e.g., "m/44'/4218'/0'/0'/0'")
 * @returns An object containing bip44CoinType, accountIndex, changeIndex, and addressIndex
 */
export function parseDerivationPath(path: string): MakeDerivationOptions {
    const parts = path
        .split("'/")
        .slice(1)
        .map((part) => parseInt(part, 10));

    return {
        bip44CoinType: parts[0],
        accountIndex: parts[1],
        changeIndex: parts[2],
        addressIndex: parts[3],
    };
}
