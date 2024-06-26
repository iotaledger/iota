// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaClient } from '@iota/iota.js/client';
import { mnemonicToSeedHex } from '@iota/iota.js/cryptography';
import { requestIotaFromFaucetV1 } from '@iota/iota.js/faucet';
import { Ed25519Keypair } from '@iota/iota.js/keypairs/ed25519';
import { entropyToMnemonic, getRandomEntropy } from '_shared/utils/bip39';
import { recoverAccounts } from './accounts-finder';
import { test, assert } from 'vitest';
import { AccountFromFinder } from '_src/shared/accounts';

const GAS_TYPE_ARG = '0x2::iota::IOTA';
const IOTA_COIN_TYPE = 4218; // 4219 for Shimmer

test('existing accounts', async () => {
    const client = new IotaClient({
        url: 'http://127.0.0.1:9000',
    });
    const faucetUrl = 'http://127.0.0.1:9123';

    const coinType = IOTA_COIN_TYPE;
    const mnemonic =
        'power dragon mercy range fee book twenty cash room coil trend first seed apple accuse purity remain rather tip use card sock south retreat';
    const seed = mnemonicToSeedHex(mnemonic);

    // First time get funds to two addresses
    const firstAddress = Ed25519Keypair.deriveKeypairFromSeed(seed, `m/44'/${coinType}'/0'/0'/0'`)
        .getPublicKey()
        .toIotaAddress();
    const objects = await client.getOwnedObjects({ owner: firstAddress });
    if (objects.data.length == 0) {
        await requestIotaFromFaucetV1({ host: faucetUrl, recipient: firstAddress });
        await requestIotaFromFaucetV1({
            host: faucetUrl,
            recipient: Ed25519Keypair.deriveKeypair(mnemonic, `m/44'/${coinType}'/1'/0'/15'`)
                .getPublicKey()
                .toIotaAddress(),
        });
    }

    let accounts: AccountFromFinder[] = [
        {
            index: 0,
            addresses: [],
        },
    ];

    accounts = await recoverAccounts(0, 0, 1, accounts, seed, coinType, client, GAS_TYPE_ARG);
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

    accounts = await recoverAccounts(0, 1, 16, accounts, seed, coinType, client, GAS_TYPE_ARG);
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
    const client = new IotaClient({
        url: 'http://127.0.0.1:9000',
    });

    const coinType = IOTA_COIN_TYPE;
    const mnemonic = entropyToMnemonic(getRandomEntropy());
    const seed = mnemonicToSeedHex(mnemonic);

    let accounts: AccountFromFinder[] = [];

    accounts = await recoverAccounts(0, 0, 10, accounts, seed, coinType, client, GAS_TYPE_ARG);
    let expectedAccounts = 0;
    assert(
        accounts.length == expectedAccounts,
        `accounts length mismatch ${accounts.length}/${expectedAccounts}`,
    );

    accounts = await recoverAccounts(0, 1, 10, accounts, seed, coinType, client, GAS_TYPE_ARG);
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

    accounts = await recoverAccounts(0, 5, 5, accounts, seed, coinType, client, GAS_TYPE_ARG);
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
