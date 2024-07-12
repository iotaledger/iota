// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { CoinBalance } from '@iota/iota.js/client';
import type { MakeDerivationOptions } from '_src/background/account-sources/bip44Path';
import { getAccountSourceByID } from '_src/background/account-sources';
import { addNewAccounts, getAccountsByAddress } from '../accounts';
import { type SerializedAccount } from '../accounts/Account';

export const getEmptyBalance = (coinType: string): CoinBalance => ({
    coinType: coinType,
    coinObjectCount: 0,
    totalBalance: '0',
    lockedBalance: {},
});

export async function getPublicKey(options: { sourceID: string } & MakeDerivationOptions) {
    const accountSource = await getAccountSourceByID(options.sourceID);

    if (!accountSource) {
        throw new Error('Could not find account source');
    }

    const pubKey = await accountSource.derivePubKey(options);
    return pubKey?.toIotaAddress();
}

// Derive all the accounts given the addresses bip paths
// and they get persisted under the account source ID that is passed.
export async function persistAddressesToSource(
    accountSourceID: string,
    addressesBipPaths: MakeDerivationOptions[],
) {
    const accountSource = await getAccountSourceByID(accountSourceID);

    if (!accountSource) {
        throw new Error('Could not find account source');
    }

    // Get derived accounts by their bip path
    const derivedAccounts: Omit<SerializedAccount, 'id'>[] = await Promise.all(
        addressesBipPaths.map((addressBipPath) => accountSource.deriveAccount(addressBipPath)),
    );

    // Filter those accounts that already exist so they are not duplicated
    const derivedAccountsNonExistent: Omit<SerializedAccount, 'id'>[] = (
        await Promise.all(
            derivedAccounts.map(async (account) => {
                const foundAccounts = await getAccountsByAddress(account.address);
                for (const foundAccount of foundAccounts) {
                    if (foundAccount.type === account.type) {
                        // Do not persist accounts with the same address and type
                        return undefined;
                    }
                }

                return account;
            }),
        )
    ).filter(Boolean) as Omit<SerializedAccount, 'id'>[];

    // Actually persist the accounts
    await addNewAccounts(derivedAccountsNonExistent);
}
