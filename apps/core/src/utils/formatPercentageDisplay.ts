// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// For unavailable %, return '--' else return the APY number
export function formatPercentageDisplay(
    value: number | null,
    nullDisplay = '--',
    isApyApprox = false,
) {
    return value === null || value === 0 ? nullDisplay : `${isApyApprox ? '~' : ''}${value}%`;
}
