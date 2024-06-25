// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type CoinBalance, type IotaClient } from '@iota/iota.js/client';
import { Ed25519Keypair } from '@iota/iota.js/keypairs/ed25519';

/**
 * Function to search for accounts/addresses with objects
 * Simplified implementation which just returns all generated accounts+addresses without deleting new generated that have no objects
 * Assumes that change, accounts and address indexes have no gaps and are ordered (so having only 0,1,3 or 0,2,1 would be invalid)
 *   @param {number} accountStartIndex The index of the first account to search for.
 *   @param {number} accountGapLimit The number of accounts to search for, after the last account with unspent outputs.
 *   TODO: addressStartIndex not used, maybe also not needed
 *   @param {number} addressStartIndex The index of the first address to search for.
 *   @param {number} addressGapLimit The number of addresses to search for, after the last address with unspent outputs, in each account.
 */
export async function recoverAccounts(
    accountStartIndex: number,
    accountGapLimit: number,
    addressStartIndex: number,
    addressGapLimit: number,
    accounts: FoundAccount[],
    seed: string,
    coinType: number,
    client: IotaClient,
    gasTypeArg: string,
): Promise<FoundAccount[]> {
    // TODO: first check that accounts: Account[] is correctly sorted, if not, throw exception or somethintg
    // Check new addresses for existing accounts
    if (addressGapLimit > 0) {
        for (const account of accounts) {
            if (account.index < accountStartIndex) {
                continue;
            }
            accounts[account.index] = await searchAddressesWithObjects(
                addressStartIndex,
                addressGapLimit,
                account,
                seed,
                coinType,
                client,
                gasTypeArg,
            );
        }
    }
    const accountsLength = accounts.length;
    let targetIndex = accountsLength + accountGapLimit;

    // Check addresses for new accounts
    for (let accountIndex = accountsLength; accountIndex < targetIndex; accountIndex += 1) {
        let account: FoundAccount = {
            index: accountIndex,
            addresses: [],
        };
        account = await searchAddressesWithObjects(
            addressStartIndex,
            addressGapLimit,
            account,
            seed,
            coinType,
            client,
            gasTypeArg,
        );
        accounts[accountIndex] = account;
        if (account.addresses.flat().find((a: Address) => hasBalance(a.balance))) {
            // Generate more accounts if something was found
            targetIndex = accountIndex + 1 + accountGapLimit;
        }
    }

    return accounts;
}

async function searchAddressesWithObjects(
    addressStartIndex: number,
    addressGapLimit: number,
    account: FoundAccount,
    seed: string,
    coinType: number,
    client: IotaClient,
    gasTypeArg: string,
): Promise<FoundAccount> {
    const accountAddressLength = account.addresses.length;
    let targetIndex = accountAddressLength + addressGapLimit;

    for (let addressIndex = accountAddressLength; addressIndex < targetIndex; addressIndex += 1) {
        const changeIndexes = [0, 1]; // in the past the change indexes were used as 0=deposit & 1=internal
        for (const changeIndex of changeIndexes) {
            const bipPath = `m/44'/${coinType}'/${account.index}'/${changeIndex}'/${addressIndex}'`;
            const pubKeyHash = Ed25519Keypair.deriveKeypairFromSeed(seed, bipPath)
                .getPublicKey()
                .toIotaAddress();

            const balance = await client.getBalance({
                owner: pubKeyHash,
                coinType: gasTypeArg,
            });

            if (hasBalance(balance)) {
                // Generate more addresses if something was found
                targetIndex = addressIndex + 1 + addressGapLimit;
            }

            if (!account.addresses[addressIndex]) {
                account.addresses[addressIndex] = [];
            }

            account.addresses[addressIndex][changeIndex] = {
                pubKeyHash,
                bipPath: {
                    addressIndex,
                    accountIndex: account.index,
                    changeIndex,
                },
                balance,
            };
        }
    }

    return account;
}

function hasBalance(balance: CoinBalance): boolean {
    return balance.coinObjectCount > 0;
}

interface Address {
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

export interface FoundAccount {
    index: number;
    addresses: Array<Array<Address>>;
    // addresses: [
    //     [change0, change1], // address index 0
    //     [change0, change1], // address index 1
    //     ...
    //  ]
}
