// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { recoverAccounts, mergeAccounts } from './utils';
import { assert, test } from 'vitest';
import { FindBalance } from './types';

const findBalanceFactory = (
    accountIndexesWithBalance: number[],
    addressIndexesWithBalance: number[],
    changeIndexesWithBalance: number[],
): FindBalance => {
    return ({ accountIndex, addressIndex, changeIndex }) => {
        if (
            accountIndexesWithBalance.includes(accountIndex) &&
            addressIndexesWithBalance.includes(addressIndex) &&
            changeIndexesWithBalance.includes(changeIndex)
        ) {
            return Promise.resolve({
                publicKey: '',
                balance: {
                    totalBalance: '100',
                    coinObjectCount: 2,
                    coinType: '0x2::iota::IOTA',
                },
            });
        }

        return Promise.resolve({
            publicKey: '',
            balance: {
                totalBalance: '0',
                coinObjectCount: 0,
                coinType: '0x2::iota::IOTA',
            },
        });
    };
};

test('BreadthSearch with not found addresses', async () => {
    const findBalance = findBalanceFactory([], [], []);

    const foundAccounts = await recoverAccounts({
        accountStartIndex: 0,
        accountGapLimit: 2,
        addressStartIndex: 0,
        addressGapLimit: 0,
        changeIndexes: [0],
        findBalance: findBalance,
    });

    assert(foundAccounts.length === 2); // expected number of accounts - 2;
    assert(foundAccounts[0].addresses.length === 1); // expected number of addresses - 1 as we provided addressGapLimit = 0;
});

test('BreadthSearch with found addresses', async () => {
    const findBalance = findBalanceFactory([0], [0], [0]);

    const foundAccounts = await recoverAccounts({
        accountStartIndex: 0,
        accountGapLimit: 2,
        addressStartIndex: 0,
        addressGapLimit: 0,
        changeIndexes: [0, 1],
        findBalance: findBalance,
    });

    assert(foundAccounts.length === 3); // expected number of accounts - 3 as we have a hit on position 0,0,0;
    assert(foundAccounts[0].addresses.length === 1); // expected number of addresses - 1 as we provided addressGapLimit = 0;
});

test('DepthSearch with found addresses', async () => {
    const findBalance = findBalanceFactory([0], [0, 2], [0]);

    const foundAccounts = await recoverAccounts({
        accountStartIndex: 0,
        accountGapLimit: 0,
        addressStartIndex: 0,
        addressGapLimit: 4,
        changeIndexes: [0, 1],
        findBalance: findBalance,
    });

    assert(foundAccounts.length === 1); // expected number of accounts - 1 as we make a search by isolated address;

    // we expect to have at least 4 addresses (indexes: 0, 1, 2, 3).
    // But as we have a hit on positions: (0,0,0), (0,2,0).
    // Because of 2 (last index with balance) + 4 (addressGapLimit) = 6 last expected index.
    // Number of elements should be 7
    assert(foundAccounts[0].addresses.length === 7);
});

test('Merge accounts', async () => {
    const findBalance = findBalanceFactory([], [], []);

    const foundAccounts1 = await recoverAccounts({
        accountStartIndex: 0,
        accountGapLimit: 3,
        addressStartIndex: 0,
        addressGapLimit: 4,
        changeIndexes: [0, 1],
        findBalance: findBalance,
    });
    const foundAccounts2 = await recoverAccounts({
        accountStartIndex: 1,
        accountGapLimit: 3,
        addressStartIndex: 0,
        addressGapLimit: 5,
        changeIndexes: [0, 1],
        findBalance: findBalance,
    });

    const mergedAccounts = mergeAccounts(foundAccounts1, foundAccounts2);

    // merged accounts count is 4 because max index for both accounts is 4.
    assert(mergedAccounts.length === 4);

    // merged accounts count is 4 because max index for both accounts is 5.
    assert(mergedAccounts[0].addresses.length === 4); // for account 0 max number of address is 4;
    assert(mergedAccounts[1].addresses.length === 5); // for account 1 max number of address is 5;
});
