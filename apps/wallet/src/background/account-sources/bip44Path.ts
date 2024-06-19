// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export interface Bip44Path {
    accountIndex: number;
    addressIndex?: number;
}

export function makeDerivationPath({ accountIndex, addressIndex }: Bip44Path) {
    // currently returns only Ed25519 path
    return `m/44'/4218'/${accountIndex}'/0'/${addressIndex || 0}'`;
}
