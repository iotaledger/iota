// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { CoinBalance } from '@iota/iota-sdk/client';

export type FindBalance = (params: {
    accountIndex: number;
    addressIndex: number;
    changeIndex: number;
}) => Promise<{
    publicKey: string;
    balance: CoinBalance;
    hasAssets: boolean;
    hasTimelockedObjects: boolean;
    hasStardustObjects: boolean;
}>;
