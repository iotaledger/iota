// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export function formatApy(apy: number, isApyApproxZero: boolean = false): string {
    return isApyApproxZero ? '~ 0%' : `${apy}%`;
}
