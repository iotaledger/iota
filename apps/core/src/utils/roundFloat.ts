// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const DEFAULT_PRECISION = 2;
export function roundFloat(num: number, precision = DEFAULT_PRECISION) {
    return parseFloat(num.toFixed(precision));
}
