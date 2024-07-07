// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type GetBalanceParams, type CoinBalance } from '@iota/iota.js/client';
import { type AccountFromFinder, type AddressFromFinder } from '_src/shared/accounts';
import { type MakeDerivationOptions } from '../account-sources/bip44Path';

type GetBalance = (
    makeDerivationOptions: MakeDerivationOptions,
    params: GetBalanceParams,
) => Promise<CoinBalance>;
export type GetPublicKey = (makeDerivationOptions: MakeDerivationOptions) => Promise<string>;

/**
 * Function to search for accounts/addresses with objects
 * Simplified implementation which just returns all generated accounts+addresses without deleting new generated that have no objects
 * Assumes that change, accounts and address indexes have no gaps and are ordered (so having only 0,1,3 or 0,2,1 would be invalid)
 *   @param {number} accountStartIndex The index of the first account to search for.
 *   @param {number} accountGapLimit The number of accounts to search for, after the last account with unspent outputs.
 *   @param {number} addressGapLimit The number of addresses to search for, after the last address with unspent outputs, in each account.
 *   @param {AccountFromFinder[]} accounts Array buffer to store the found accounts.
 *   @param {number} coinType Coin ID to be used.
 *   @param {GetBalance} getBalance Callback to retrieve a balance of a given address, usually by using the IotaClient.
 *   @param {gasTypeArg} gasTypeArg The Coin type name of Move.
 *   @param {GetPublicKey} getPublicKey Callback to retrieve the public key of the keypair in a given bippath.
 */
export async function findAccounts(
    accountStartIndex: number,
    accountGapLimit: number,
    addressGapLimit: number,
    accounts: AccountFromFinder[],
    coinType: number,
    getBalance: GetBalance,
    gasTypeArg: string,
    getPublicKey: GetPublicKey,
): Promise<AccountFromFinder[]> {
    // TODO: first check that accounts: Account[] is correctly sorted, if not, throw exception or something
    // Check new addresses for existing accounts
    if (addressGapLimit > 0) {
        for (const account of accounts) {
            if (account.index < accountStartIndex) {
                continue;
            }
            accounts[account.index] = await searchAddressesWithObjects(
                addressGapLimit,
                account,
                coinType,
                getBalance,
                gasTypeArg,
                getPublicKey,
            );
        }
    }
    const accountsLength = accounts.length;
    let targetIndex = accountsLength + accountGapLimit;

    // Check addresses for new accounts
    for (let accountIndex = accountsLength; accountIndex < targetIndex; accountIndex += 1) {
        let account: AccountFromFinder = {
            index: accountIndex,
            addresses: [],
        };
        account = await searchAddressesWithObjects(
            addressGapLimit,
            account,
            coinType,
            getBalance,
            gasTypeArg,
            getPublicKey,
        );
        accounts[accountIndex] = account;
        if (account.addresses.flat().find((a: AddressFromFinder) => hasBalance(a.balance))) {
            // Generate more accounts if something was found
            targetIndex = accountIndex + 1 + accountGapLimit;
        }
    }

    return accounts;
}

async function searchAddressesWithObjects(
    addressGapLimit: number,
    account: AccountFromFinder,
    coinType: number,
    getBalance: GetBalance,
    gasTypeArg: string,
    getPublicKey: GetPublicKey,
): Promise<AccountFromFinder> {
    const accountAddressesLength = account.addresses.length;
    let targetIndex = accountAddressesLength + addressGapLimit;

    for (let addressIndex = accountAddressesLength; addressIndex < targetIndex; addressIndex += 1) {
        const changeIndexes = [0, 1]; // in the past the change indexes were used as 0=deposit & 1=internal
        for (const changeIndex of changeIndexes) {
            const bipPath = {
                coinType,
                accountIndex: account.index,
                addressIndex,
                changeIndex,
            };
            const pubKeyHash = await getPublicKey(bipPath);

            const balance = await getBalance(bipPath, {
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

export function hasBalance(balance: CoinBalance): boolean {
    return balance.coinObjectCount > 0;
}
