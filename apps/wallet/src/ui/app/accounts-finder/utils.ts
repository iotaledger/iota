// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type CoinBalance } from '@iota/iota-sdk/client';
import type { AccountFromFinder, AddressFromFinder } from '_src/shared/accounts';
import type { FindBalance } from './types';

/**
 * Recover accounts function and all related interfaces
 */
export interface RecoverAccountsParams {
    accountStartIndex: number;
    accountGapLimit: number;
    addressStartIndex: number;
    addressGapLimit: number;
    changeIndexes: number[];
    findBalance: FindBalance;
}

export async function recoverAccounts(params: RecoverAccountsParams): Promise<AccountFromFinder[]> {
    const {
        accountStartIndex,
        accountGapLimit,
        addressStartIndex,
        addressGapLimit,
        changeIndexes,
        findBalance,
    } = params;

    const accounts: AccountFromFinder[] = [];

    // isolated search for one account;
    if (!accountGapLimit) {
        const { account } = await recoverAccount({
            accountIndex: accountStartIndex,
            addressStartIndex,
            addressGapLimit,
            changeIndexes,
            findBalance,
        });
        accounts.push(account);
        return accounts;
    }

    // we search for accounts in the given range
    let targetAccountIndex = accountStartIndex + accountGapLimit;
    for (let accountIndex = accountStartIndex; accountIndex < targetAccountIndex; accountIndex++) {
        const accountData = await recoverAccount({
            accountIndex,
            addressStartIndex,
            addressGapLimit,
            changeIndexes,
            findBalance,
        });

        // if any of the addresses of the given account has a balance,
        // we increase the target account index to keep searching
        if (accountData.isBalanceExists) {
            targetAccountIndex = accountIndex + accountGapLimit + 1;
        }
        // we add the account to the list of accounts
        accounts.push(accountData.account);
    }
    return accounts;
}

/**
 * Recover account by index
 */
async function recoverAccount(
    params: {
        accountIndex: number;
    } & Pick<
        RecoverAccountsParams,
        'addressStartIndex' | 'addressGapLimit' | 'changeIndexes' | 'findBalance'
    >,
) {
    const { accountIndex, addressStartIndex, addressGapLimit, changeIndexes, findBalance } = params;
    const account: AccountFromFinder = {
        index: accountIndex,
        addresses: [],
    };

    // Flag to check if any of the addresses of the account has a balance
    let isBalanceExists = false;

    // Isolated search for no address rotation
    if (!addressGapLimit) {
        const { addresses, isBalanceExists: isBalanceExists } = await searchBalances({
            accountIndex,
            addressIndex: addressStartIndex,
            changeIndexes,
            findBalance,
        });

        account.addresses.push(addresses); // we add the addresses to the account

        return {
            account,
            isBalanceExists,
        };
    }

    // on each fixed account index, we search for addresses in the given range
    let targetAddressIndex = addressStartIndex + addressGapLimit;
    for (let addressIndex = addressStartIndex; addressIndex < targetAddressIndex; addressIndex++) {
        const { addresses, isBalanceExists: isHasBalance } = await searchBalances({
            accountIndex,
            addressIndex,
            changeIndexes,
            findBalance,
        });

        if (isHasBalance) {
            targetAddressIndex = addressIndex + addressGapLimit + 1;
            isBalanceExists = true;
        }

        account.addresses.push(addresses);
    }

    return { account, isBalanceExists };
}

/**
 * Search balances for the given account, address and change indexes
 * @return list of addresses with balances and flag if any of the addresses has a balance
 */
async function searchBalances({
    accountIndex,
    addressIndex,
    changeIndexes,
    findBalance,
}: {
    accountIndex: number;
    addressIndex: number;
} & Pick<RecoverAccountsParams, 'changeIndexes' | 'findBalance'>) {
    const addresses: AddressFromFinder[] = [];

    // if any of the addresses has a balance, we increase the target address index to keep searching
    let isBalanceExists = false;
    for (const changeIndex of changeIndexes) {
        const foundBalance = await findBalance({
            accountIndex,
            addressIndex,
            changeIndex,
        });

        if (hasBalance(foundBalance.balance)) {
            isBalanceExists = true;
        }

        addresses.push({
            publicKey: foundBalance.publicKey,
            balance: foundBalance.balance,
            bipPath: {
                addressIndex,
                accountIndex,
                changeIndex,
            },
            hasTimelockedObjects: foundBalance.hasAssets,
            hasStardustObjects: foundBalance.hasTimelockedObjects,
            hasAssets: foundBalance.hasStardustObjects,
        });
    }
    return {
        addresses,
        isBalanceExists,
    };
}

export function hasBalance(balance: CoinBalance): boolean {
    return balance.coinObjectCount > 0;
}

// Transform list of accounts and found balances to format.
// This function allow to remove duplicates in the list of accounts.
// {
//   'addressIndex-accountIndex-changeIndex': 'AddressFromFinder
// }
function transformToBipMap(accounts: AccountFromFinder[]) {
    const bipMap: Record<string, AddressFromFinder> = {};
    accounts.forEach((account) => {
        account.addresses.forEach((address) => {
            address.forEach((changeIndexObj) => {
                const { accountIndex, addressIndex, changeIndex } = changeIndexObj.bipPath;
                const key = `${accountIndex}-${addressIndex}-${changeIndex}`;
                bipMap[key] = changeIndexObj;
            });
        });
    });
    return bipMap;
}

// Transform bipMap to list of accounts back.
function transformFromBipMap(bipMap: Record<string, AddressFromFinder>) {
    let accounts: AccountFromFinder[] = [];

    Object.entries(bipMap).forEach(([key, address]) => {
        const [accountIndex, addressIndex, changeIndex] = key.split('-').map(Number);

        // add empty accounts if they don't exist
        if (!accounts[accountIndex]) {
            accounts[accountIndex] = {
                index: accountIndex,
                addresses: [],
            };
        }

        // add empty addresses if they don't exist
        if (!accounts[accountIndex].addresses[addressIndex]) {
            accounts[accountIndex].addresses[addressIndex] = [];
        }

        accounts[accountIndex].addresses[addressIndex][changeIndex] = address;
    });

    accounts = accounts.filter((account) => !!account); // empty accounts possible when accountIndex is custom. => [empty x 100, {index: 101...}]
    return accounts;
}

// Merge two lists of accounts and remove duplicates.
export function mergeAccounts(accounts1: AccountFromFinder[], accounts2: AccountFromFinder[]) {
    const bipMap = transformToBipMap([...accounts1, ...accounts2]);
    return transformFromBipMap(bipMap);
}

// Diff the found and persisted accounts so we know
// what addresses have not persisted yet and have relevant assets or balance.
export function diffAddressesBipPaths(
    foundAccounts: AccountFromFinder[],
    persistedAccounts: AccountFromFinder[],
): AddressFromFinder[] {
    const foundBipMap = transformToBipMap(foundAccounts);
    const persistedBipMap = transformToBipMap(persistedAccounts);

    const foundBipMapKeys = Object.entries(foundBipMap);
    const persistedBipMapKeys = Object.keys(persistedBipMap);
    const diffBipPaths = foundBipMapKeys.filter(
        ([key, address]) =>
            !persistedBipMapKeys.includes(key) &&
            (hasBalance(address.balance) ||
                address.hasAssets ||
                address.hasTimelockedObjects ||
                address.hasStardustObjects),
    );

    return diffBipPaths.map(([_, account]) => account);
}
