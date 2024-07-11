// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { CoinBalance } from '@iota/iota.js/client';

export type FindBalance = (params: {
    accountIndex: number;
    addressIndex: number;
    changeIndex: number;
}) => Promise<{
    publicKeyHash: string;
    balance: CoinBalance;
}>;
