// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export function formatBalanceToUSD(balance: number): string {
    return balance.toLocaleString('en', {
        style: 'currency',
        currency: 'USD',
    });
}
