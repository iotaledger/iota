// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { CoinBalance, IotaClient } from '@iota/iota.js/client';
import {
    makeDerivationPath,
    type MakeDerivationOptions,
} from '_src/background/account-sources/bip44Path';
import { getAccountSourceByID } from '_src/background/account-sources';
import { addNewAccounts, getAccountsByAddress } from '../../../background/accounts';
import { type SerializedAccount } from '../../../background/accounts/Account';
import { LedgerAccount } from '../../../background/accounts/LedgerAccount';

export const getEmptyBalance = (coinType: string): CoinBalance => ({
    coinType: coinType,
    coinObjectCount: 0,
    totalBalance: '0',
    lockedBalance: {},
});

// // Derive all the accounts given the addresses bip paths
// // and they get persisted under the account source ID that is passed.
// export async function persistAddressesToSource(
//     sourceStrategy: SourceStrategy,
//     addressesBipPaths: MakeDerivationOptions[],
//     client: IotaClient
// ) {
//     let derivedAccounts: Omit<SerializedAccount, 'id'>[] = []

//     switch (sourceStrategy.type){
//         case 'mnemonic':
//         case 'seed':
//             const accountSource = await getAccountSourceByID(sourceStrategy.sourceID);

//             if (!accountSource) {
//                 throw new Error('Could not find account source');
//             }

//             derivedAccounts =  await Promise.all(
//                 addressesBipPaths.map((addressBipPath) => accountSource.deriveAccount(addressBipPath)),
//             );
//             break;
//         case 'ledger':
//             derivedAccounts = await Promise.all(addressesBipPaths.map(async (addressBipPath) => {
//                 const derivationPath = makeDerivationPath(addressBipPath);
//                 const publicKeyResult = await client.getPublicKey(derivationPath);
//                 const publicKey = new Ed25519PublicKey(publicKeyResult.publicKey);
//                 const iotaAddress = publicKey.toIotaAddress();
//                 return LedgerAccount.createNew({
//                     password: sourceStrategy.password,
//                     address
//                 })
//             }));

//     }

//     // Filter those accounts that already exist so they are not duplicated
//     const derivedAccountsNonExistent: Omit<SerializedAccount, 'id'>[] = (
//         await Promise.all(
//             derivedAccounts.map(async (account) => {
//                 const foundAccounts = await getAccountsByAddress(account.address);
//                 for (const foundAccount of foundAccounts) {
//                     if (foundAccount.type === account.type) {
//                         // Do not persist accounts with the same address and type
//                         return undefined;
//                     }
//                 }

//                 return account;
//             }),
//         )
//     ).filter(Boolean) as Omit<SerializedAccount, 'id'>[];

//     // Actually persist the accounts
//     await addNewAccounts(derivedAccountsNonExistent);
// }
