// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createMessage, type Message } from '_src/shared/messaging/messages';
import {
    isMethodPayload,
    type MethodPayload,
} from '_src/shared/messaging/messages/payloads/methodPayload';
import { type WalletStatusChange } from '_src/shared/messaging/messages/payloads/wallet-status-change';
import { fromB64 } from '@iota/iota-sdk/utils';
import Dexie from 'dexie';
import { getAccountSourceByID } from '../account-sources';
import { accountSourcesEvents } from '../account-sources/events';
import { MnemonicAccountSource } from '../account-sources/mnemonicAccountSource';
import { SeedAccountSource } from '../account-sources/seedAccountSource';
import { type UiConnection } from '../connections/uiConnection';
import { backupDB, getDB } from '../db';
import { makeUniqueKey } from '../storageUtils';
import {
    AccountType,
    isKeyPairExportableAccount,
    isPasswordUnLockable,
    isSigningAccount,
    type SerializedAccount,
} from './account';
import { accountsEvents } from './events';
import { ImportedAccount } from './importedAccount';
import { LedgerAccount } from './ledgerAccount';
import { MnemonicAccount } from './mnemonicAccount';
import { SeedAccount } from './seedAccount';
import { MILLISECONDS_PER_SECOND, SECONDS_PER_MINUTE } from '@iota/core';

function toAccount(account: SerializedAccount) {
    if (MnemonicAccount.isOfType(account)) {
        return new MnemonicAccount({ id: account.id, cachedData: account });
    }
    if (SeedAccount.isOfType(account)) {
        return new SeedAccount({ id: account.id, cachedData: account });
    }
    if (ImportedAccount.isOfType(account)) {
        return new ImportedAccount({ id: account.id, cachedData: account });
    }
    if (LedgerAccount.isOfType(account)) {
        return new LedgerAccount({ id: account.id, cachedData: account });
    }
    throw new Error(`Unknown account of type ${account.type}`);
}

export async function getAllAccounts(filter?: { sourceID: string }) {
    const db = await getDB();
    let accounts;
    if (filter?.sourceID) {
        accounts = await db.accounts.where('sourceID').equals(filter.sourceID).sortBy('createdAt');
    } else {
        accounts = await db.accounts.toCollection().sortBy('createdAt');
    }
    return accounts.map(toAccount);
}

export async function getAccountByID(id: string) {
    const serializedAccount = await (await getDB()).accounts.get(id);
    if (!serializedAccount) {
        return null;
    }
    return toAccount(serializedAccount);
}

export async function getAccountsByAddress(address: string) {
    return (await (await getDB()).accounts.where('address').equals(address).toArray()).map(
        toAccount,
    );
}

export async function getAllSerializedUIAccounts() {
    return Promise.all((await getAllAccounts()).map((anAccount) => anAccount.toUISerialized()));
}

export async function isAccountsInitialized() {
    return (await (await getDB()).accounts.count()) > 0;
}

export async function getAccountsStatusData(
    accountsFilter?: string[],
): Promise<Required<WalletStatusChange>['accounts']> {
    const allAccounts = await (await getDB()).accounts.toArray();
    return allAccounts
        .filter(({ address }) => !accountsFilter || accountsFilter.includes(address))
        .map(({ address, publicKey, nickname }) => ({ address, publicKey, nickname }));
}

export async function changeActiveAccount(accountID: string) {
    const db = await getDB();
    return db.transaction('rw', db.accounts, async () => {
        const newSelectedAccount = await db.accounts.get(accountID);
        if (!newSelectedAccount) {
            throw new Error(`Failed, account with id ${accountID} not found`);
        }
        await db.accounts.where('id').notEqual(accountID).modify({ selected: false });
        await db.accounts.update(accountID, { selected: true });
        accountsEvents.emit('activeAccountChanged', { accountID });
    });
}

export async function addNewAccounts<T extends SerializedAccount>(accounts: Omit<T, 'id'>[]) {
    const db = await getDB();
    const accountsCreated = await db.transaction('rw', db.accounts, async () => {
        const accountInstances = [];
        for (const anAccountToAdd of accounts) {
            let id = '';
            const existingSameAddressAccounts = await getAccountsByAddress(anAccountToAdd.address);
            for (const anExistingAccount of existingSameAddressAccounts) {
                if (
                    (await Dexie.waitFor(anExistingAccount.address)) === anAccountToAdd.address &&
                    anExistingAccount.type === anAccountToAdd.type
                ) {
                    // allow importing accounts that have the same address but are of different type
                    // probably it's an edge case and we used to see this problem with importing
                    // accounts that were exported from the mnemonic while testing
                    throw new Error(`Duplicated account ${anAccountToAdd.address}`);
                }
            }
            id = id || makeUniqueKey();
            await db.accounts.put({ ...anAccountToAdd, id });
            const accountInstance = await Dexie.waitFor(getAccountByID(id));
            if (!accountInstance) {
                throw new Error(`Something went wrong account with id ${id} not found`);
            }
            accountInstances.push(accountInstance);
        }
        const selectedAccount = await db.accounts.filter(({ selected }) => selected).first();
        if (!selectedAccount && accountInstances.length) {
            const firstAccount = accountInstances[0];
            await db.accounts.update(firstAccount.id, { selected: true });
        }
        return accountInstances;
    });
    await backupDB();
    accountsEvents.emit('accountsChanged');
    return accountsCreated;
}

export async function lockAllAccounts() {
    const allAccounts = await getAllAccounts();
    for (const anAccount of allAccounts) {
        await anAccount.lock();
    }
}

interface LockedState {
    failedAttempts: number;
    lastFailedAttemptTime: number | null;
    isLockedOut: boolean;
    lockTimeMs: number | null;
}

async function getLockedState() {
    const db = await getDB();
    const lockTimeMs = await db.settings.get('lockTimeMs');
    const failedAttempts = await db.settings.get('failedAttempts');
    const lastFailedAttemptTime = await db.settings.get('lastFailedAttemptTime');
    const isLockedOut = await db.settings.get('isLockedOut');

    return {
        failedAttempts: failedAttempts?.value || 0,
        lastFailedAttemptTime: lastFailedAttemptTime?.value || null,
        isLockedOut: isLockedOut?.value || false,
        lockTimeMs: lockTimeMs?.value || null,
    };
}

async function updateLockedState(newState: Partial<LockedState>) {
    const db = await getDB();
    await db.transaction('rw', db.settings, async () => {
        for (const [key, value] of Object.entries(newState)) {
            await db.settings.put({ setting: key, value });
        }
    });
}

async function clearStateAfterManyFailedAttempts() {
    const db = await getDB();
    await db.transaction('rw', db.settings, async () => {
        await db.settings.put({ setting: 'failedAttempts', value: 0 });
        await db.settings.put({ setting: 'lastFailedAttemptTime', value: null });
        await db.settings.put({ setting: 'isLockedOut', value: false });
        await db.settings.put({ setting: 'lockTimeMs', value: null });
    });
}

export async function accountsHandleUIMessage(msg: Message, uiConnection: UiConnection) {
    const { payload } = msg;
    if (isMethodPayload(payload, 'lockAccountSourceOrAccount')) {
        const account = await getAccountByID(payload.args.id);
        if (account) {
            await account.lock();
            await uiConnection.send(createMessage({ type: 'done' }, msg.id));
            return true;
        }
    }
    if (isMethodPayload(payload, 'setAccountNickname')) {
        const { id, nickname } = payload.args;
        const account = await getAccountByID(id);
        if (account) {
            await account.setNickname(nickname);
            await uiConnection.send(createMessage({ type: 'done' }, msg.id));
            return true;
        }
    }
    if (isMethodPayload(payload, 'unlockAccountSourceOrAccount')) {
        const { id, password } = payload.args;
        const account = await getAccountByID(id);
        if (account) {
            if (isPasswordUnLockable(account)) {
                await account.passwordUnlock(password);
            }
            await uiConnection.send(createMessage({ type: 'done' }, msg.id));
            return true;
        }
    }
    if (isMethodPayload(payload, 'signData')) {
        const { id, data } = payload.args;
        const account = await getAccountByID(id);
        if (!account) {
            throw new Error(`Account with address ${id} not found`);
        }
        if (!isSigningAccount(account)) {
            throw new Error(`Account with address ${id} is not a signing account`);
        }
        await uiConnection.send(
            createMessage<MethodPayload<'signDataResponse'>>(
                {
                    type: 'method-payload',
                    method: 'signDataResponse',
                    args: { signature: await account.signData(fromB64(data)) },
                },
                msg.id,
            ),
        );
        return true;
    }
    if (isMethodPayload(payload, 'createAccounts')) {
        const newSerializedAccounts: Omit<SerializedAccount, 'id'>[] = [];
        const { type } = payload.args;
        if (type === AccountType.MnemonicDerived) {
            const { sourceID } = payload.args;
            const accountSource = await getAccountSourceByID(payload.args.sourceID);
            if (!accountSource) {
                throw new Error(`Account source ${sourceID} not found`);
            }
            if (!(accountSource instanceof MnemonicAccountSource)) {
                throw new Error(`Invalid account source type`);
            }
            newSerializedAccounts.push(await accountSource.deriveAccount());
        } else if (type === AccountType.SeedDerived) {
            const { sourceID } = payload.args;
            const accountSource = await getAccountSourceByID(payload.args.sourceID);
            if (!accountSource) {
                throw new Error(`Account source ${sourceID} not found`);
            }
            if (!(accountSource instanceof SeedAccountSource)) {
                throw new Error(`Invalid account source type`);
            }
            newSerializedAccounts.push(await accountSource.deriveAccount());
        } else if (type === AccountType.PrivateKeyDerived) {
            newSerializedAccounts.push(await ImportedAccount.createNew(payload.args));
        } else if (type === AccountType.LedgerDerived) {
            const { password, accounts } = payload.args;
            for (const aLedgerAccount of accounts) {
                newSerializedAccounts.push(
                    await LedgerAccount.createNew({ ...aLedgerAccount, password }),
                );
            }
        } else {
            throw new Error(`Unknown accounts type to create ${type}`);
        }
        const newAccounts = await addNewAccounts(newSerializedAccounts);
        await uiConnection.send(
            createMessage<MethodPayload<'accountsCreatedResponse'>>(
                {
                    method: 'accountsCreatedResponse',
                    type: 'method-payload',
                    args: {
                        accounts: await Promise.all(
                            newAccounts.map(
                                async (aNewAccount) => await aNewAccount.toUISerialized(),
                            ),
                        ),
                    },
                },
                msg.id,
            ),
        );
        return true;
    }
    if (isMethodPayload(payload, 'switchAccount')) {
        await changeActiveAccount(payload.args.accountID);
        await uiConnection.send(createMessage({ type: 'done' }, msg.id));
        return true;
    }
    if (isMethodPayload(payload, 'verifyPassword')) {
        const MAX_UNLOCK_ATTEMPTS = 3;
        const WALLET_LOCK_DURATION_IN_MS = 60 * MILLISECONDS_PER_SECOND;
        const RESET_FAILED_ATTEMPTS_THRESHOLD_IN_MS =
            60 * SECONDS_PER_MINUTE * MILLISECONDS_PER_SECOND;

        const { lockTimeMs, isLockedOut, lastFailedAttemptTime } = await getLockedState();

        if (isLockedOut && lockTimeMs) {
            const elapsedTime = Date.now() - Number(lockTimeMs);
            const remainingTime = Math.max(0, WALLET_LOCK_DURATION_IN_MS - elapsedTime);

            if (remainingTime > 0) {
                // The wallet is still locked after the maximum number of failed attempts
                throw new Error(
                    `Too many failed attempts. Please try again in ${Math.ceil(remainingTime / MILLISECONDS_PER_SECOND)} seconds.`,
                );
            } else {
                await clearStateAfterManyFailedAttempts();
            }
        }

        try {
            const allAccounts = await getAllAccounts();
            for (const anAccount of allAccounts) {
                if (isPasswordUnLockable(anAccount)) {
                    await anAccount.verifyPassword(payload.args.password);
                    await clearStateAfterManyFailedAttempts();
                    await uiConnection.send(createMessage({ type: 'done' }, msg.id));
                    return true;
                }
            }
            throw new Error('No password protected account found');
        } catch (error) {
            // Check if the last failed attempt was too long ago
            const currentTime = Date.now();
            const lastFailedAttempt = lastFailedAttemptTime || 0;
            const timeSinceLastAttempt = currentTime - Number(lastFailedAttempt);

            if (timeSinceLastAttempt > RESET_FAILED_ATTEMPTS_THRESHOLD_IN_MS) {
                await updateLockedState({ failedAttempts: 0, lastFailedAttemptTime: currentTime });
            }

            const { failedAttempts: currentFailedAttempts } = await getLockedState();
            const failedAttempts = Number(currentFailedAttempts) + 1;
            const remainingAttempts = MAX_UNLOCK_ATTEMPTS - failedAttempts;

            if (failedAttempts >= MAX_UNLOCK_ATTEMPTS) {
                // Lock the wallet if the maximum number of failed attempts is reached
                await updateLockedState({
                    lockTimeMs: Date.now(),
                    isLockedOut: true,
                });
                const remainingSeconds = WALLET_LOCK_DURATION_IN_MS / MILLISECONDS_PER_SECOND;
                throw new Error(
                    `Too many failed attempts. Please try again in ${remainingSeconds} ${remainingSeconds === 1 ? 'second' : 'seconds'}.`,
                );
            } else {
                // Update the failed attempts count and the time of the last failed attempt
                await updateLockedState({ failedAttempts, lastFailedAttemptTime: currentTime });
                throw new Error(
                    `Incorrect password. You have ${remainingAttempts} ${remainingAttempts === 1 ? 'attempt' : 'attempts'} left.`,
                );
            }
        }
    }
    if (isMethodPayload(payload, 'storeLedgerAccountsPublicKeys')) {
        const { publicKeysToStore } = payload.args;
        const db = await getDB();
        // TODO: seems bulkUpdate is supported from v4.0.1-alpha.6 change to it when available
        await db.transaction('rw', db.accounts, async () => {
            for (const { accountID, publicKey } of publicKeysToStore) {
                await db.accounts.update(accountID, { publicKey });
            }
        });
        return true;
    }
    if (isMethodPayload(payload, 'getAccountKeyPair')) {
        const { password, accountID } = payload.args;
        const account = await getAccountByID(accountID);
        if (!account) {
            throw new Error(`Account with id ${accountID} not found.`);
        }
        if (!isKeyPairExportableAccount(account)) {
            throw new Error(`Cannot export account with id ${accountID}.`);
        }
        await uiConnection.send(
            createMessage<MethodPayload<'getAccountKeyPairResponse'>>(
                {
                    type: 'method-payload',
                    method: 'getAccountKeyPairResponse',
                    args: {
                        accountID: account.id,
                        keyPair: await account.exportKeyPair(password),
                    },
                },
                msg.id,
            ),
        );
        return true;
    }
    if (isMethodPayload(payload, 'removeAccount')) {
        const { accountID } = payload.args;
        const db = await getDB();
        await db.transaction('rw', db.accounts, db.accountSources, async () => {
            const account = await db.accounts.get(accountID);
            if (!account) {
                throw new Error(`Account with id ${accountID} not found.`);
            }
            const accountSourceID =
                'sourceID' in account && typeof account.sourceID === 'string' && account.sourceID;
            await db.accounts.delete(account.id);
            if (accountSourceID) {
                const totalSameSourceAccounts = await db.accounts
                    .where('sourceID')
                    .equals(accountSourceID)
                    .count();
                if (totalSameSourceAccounts === 0) {
                    await db.accountSources.delete(accountSourceID);
                }
            }
        });
        await backupDB();
        accountsEvents.emit('accountsChanged');
        accountSourcesEvents.emit('accountSourcesChanged');
        await uiConnection.send(createMessage({ type: 'done' }, msg.id));
        return true;
    }
    return false;
}
