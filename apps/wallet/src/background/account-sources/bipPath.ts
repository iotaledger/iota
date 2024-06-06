// Copyright (c) 2024 IOTA Stiftung
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export function makeDerivationPath(index: number) {
    // currently returns only Ed25519 path
    return `m/44'/4218'/${index}'/0'/0'`;
}
