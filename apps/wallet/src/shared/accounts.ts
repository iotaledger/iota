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

export class AccountTooManyAttemptsError extends Error {
    public remainingTime: number;

    constructor(remainingTime: number) {
        super(
            JSON.stringify({
                remainingTime,
            }),
        );
        this.remainingTime = remainingTime;
    }

    static fromError(error: Error) {
        try {
            const message = JSON.parse(error.message);
            if ('remainingTime' in message) {
                return new AccountTooManyAttemptsError(Number(message.remainingTime));
            }
            return null;
        } catch {
            return null;
        }
    }
}
