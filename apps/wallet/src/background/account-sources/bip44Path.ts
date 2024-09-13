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
