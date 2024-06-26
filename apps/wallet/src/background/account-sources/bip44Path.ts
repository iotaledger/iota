// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export interface MakeDerivationOptions {
    coinType?: number;
    accountIndex: number;
    addressIndex?: number;
    changeIndex?: number;
}

export function makeDerivationPath({
    coinType = 4218,
    accountIndex,
    addressIndex = 0,
    changeIndex = 0,
}: MakeDerivationOptions) {
    // currently returns only Ed25519 path
    return `m/44'/${coinType}'/${accountIndex}'/${changeIndex}'/${addressIndex}'`;
}
