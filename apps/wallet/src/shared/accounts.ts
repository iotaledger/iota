// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type CoinBalance } from '@iota/iota.js/client';

export interface AddressFromFinder {
    pubKeyHash: string;
    bipPath: Bip44Path;
    // TODO: extend this balance in the future to include eg staking, vesting schedules, assets, ...
    balance: CoinBalance;
}

export interface Bip44Path {
    accountIndex: number;
    addressIndex: number;
    changeIndex: number;
}

export interface AccountFromFinder {
    index: number;
    addresses: Array<Array<AddressFromFinder>>;
    // addresses: [
    //     [change0, change1], // address index 0
    //     [change0, change1], // address index 1
    //     ...
    //  ]
}
