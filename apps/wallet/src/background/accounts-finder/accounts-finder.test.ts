// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CoinBalance } from '@iota/iota.js/client';
import { mnemonicToSeedHex } from '@iota/iota.js/cryptography';
import { entropyToMnemonic, getRandomEntropy } from '_shared/utils/bip39';
import { findAccounts } from './accounts-finder';
import { test, assert } from 'vitest';
import { AccountFromFinder } from '_src/shared/accounts';

const GAS_TYPE_ARG = '0x2::iota::IOTA';
const IOTA_COIN_TYPE = 4218; // 4219 for Shimmer

test('existing accounts', async () => {
    const coinType = IOTA_COIN_TYPE;
    const mnemonic =
        'power dragon mercy range fee book twenty cash room coil trend first seed apple accuse purity remain rather tip use card sock south retreat';
    const seed = mnemonicToSeedHex(mnemonic);

    const getBalance = (bipPath: string): Promise<CoinBalance> => {
        let balance: CoinBalance = {
            totalBalance: '0',
            coinType: '4218',
            coinObjectCount: 0,
            lockedBalance: {},
        };
        if (
            bipPath === `m/44'/${coinType}'/1'/0'/15'` ||
            bipPath === `m/44'/${coinType}'/0'/0'/0'`
        ) {
            balance = {
                totalBalance: '100000000',
                coinType: '4218',
                coinObjectCount: 1,
                lockedBalance: {},
            };
        }
        return Promise.resolve(balance);
    };

    let accounts: AccountFromFinder[] = [
        {
            index: 0,
            addresses: [],
        },
    ];

    accounts = await findAccounts(0, 0, 1, accounts, seed, coinType, getBalance, GAS_TYPE_ARG);
    let expectedAccounts = 1;
    assert(
        accounts.length == expectedAccounts,
        `accounts length mismatch ${accounts.length}/${expectedAccounts}`,
    );
    let expectedAddresses = 2;
    assert(
        accounts[0].addresses.length == expectedAddresses,
        `addresses length mismatch ${accounts[0].addresses.length}/${expectedAddresses}`,
    );

    accounts = await findAccounts(0, 1, 16, accounts, seed, coinType, getBalance, GAS_TYPE_ARG);
    assert(accounts.length == accounts[accounts.length - 1].index + 1, `accounts length mismatch`);
    expectedAccounts = 3;
    assert(
        accounts.length == expectedAccounts,
        `accounts length mismatch ${accounts.length}/${expectedAccounts}`,
    );
    expectedAddresses = 18;
    assert(
        accounts[0].addresses.length == expectedAddresses,
        `addresses length mismatch ${accounts[0].addresses.length}/${expectedAddresses}`,
    );
    expectedAddresses = 32;
    assert(
        accounts[1].addresses.length == expectedAddresses,
        `addresses length mismatch ${accounts[1].addresses.length}/${expectedAddresses}`,
    );
});

// One existing account without object
test('empty accounts', async () => {
    const coinType = IOTA_COIN_TYPE;
    const mnemonic = entropyToMnemonic(getRandomEntropy());
    const seed = mnemonicToSeedHex(mnemonic);

    const getBalance = (): Promise<CoinBalance> => {
        return Promise.resolve({
            totalBalance: '0',
            coinType: '4218',
            coinObjectCount: 0,
            lockedBalance: {},
        });
    };

    let accounts: AccountFromFinder[] = [];

    accounts = await findAccounts(0, 0, 10, accounts, seed, coinType, getBalance, GAS_TYPE_ARG);
    let expectedAccounts = 0;
    assert(
        accounts.length == expectedAccounts,
        `accounts length mismatch ${accounts.length}/${expectedAccounts}`,
    );

    accounts = await findAccounts(0, 1, 10, accounts, seed, coinType, getBalance, GAS_TYPE_ARG);
    expectedAccounts = 1;
    assert(
        accounts.length == expectedAccounts,
        `accounts length mismatch ${accounts.length}/${expectedAccounts}`,
    );
    let expectedAddresses = 10;
    assert(
        accounts[0].addresses.length == expectedAddresses,
        `addresses length mismatch ${accounts[0].addresses.length}/${expectedAddresses}`,
    );

    accounts = await findAccounts(0, 5, 5, accounts, seed, coinType, getBalance, GAS_TYPE_ARG);
    expectedAccounts = 6;
    assert(
        accounts.length == expectedAccounts,
        `accounts length mismatch ${accounts.length}/${expectedAccounts}`,
    );
    expectedAddresses = 15;
    assert(
        accounts[0].addresses.length == expectedAddresses,
        `addresses length mismatch ${accounts[0].addresses.length}/${expectedAddresses}`,
    );
});
