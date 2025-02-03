// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type CoinBalance } from '@iota/iota-sdk/client';

export interface AddressFromFinder {
    publicKey: string;
    bipPath: Bip44Path;
    hasTimelockedObjects: boolean;
    hasStardustObjects: boolean;
    hasAssets: boolean;
    balance: CoinBalance;
}

export interface Bip44Path {
    accountIndex: number;
    addressIndex: number;
    changeIndex: number;
}

export interface AccountFromFinder {
    index: number;
    /**
     * - Example structure of 'addresses':
     *    [
     *       [change0, change1], // 'change0' and 'change1' are addresses for the account at index 0
     *       [change0, change1], // 'change0' and 'change1' are addresses for the account at index 1
     *       ...
     *    ]
     */
    addresses: Array<Array<AddressFromFinder>>;
}
