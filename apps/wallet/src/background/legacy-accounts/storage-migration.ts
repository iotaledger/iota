// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import Dexie from 'dexie';

import { accountSourcesEvents } from '../account-sources/events';
import {
	deriveKeypairFromSeed,
	MnemonicAccountSource,
} from '../account-sources/MnemonicAccountSource';
import { type SerializedAccount } from '../accounts/Account';
import { accountsEvents } from '../accounts/events';
import { ImportedAccount } from '../accounts/ImportedAccount';
import { LedgerAccount } from '../accounts/LedgerAccount';
import { MnemonicAccount } from '../accounts/MnemonicAccount';
import { backupDB, getDB } from '../db';
import { getFromLocalStorage, makeUniqueKey, setToLocalStorage } from '../storage-utils';
import { LegacyVault } from './LegacyVault';
import { makeDerivationPath } from '../account-sources/bipPath';

export type Status = 'required' | 'inProgress' | 'ready';

const migrationDoneStorageKey = 'storage-migration-done';
const storageActiveAccountKey = 'active_account';

function getActiveAccountAddress() {
	return getFromLocalStorage<string>(storageActiveAccountKey);
}

let statusCache: Status | null = null;

export async function getStatus() {
	if (statusCache) {
		return statusCache;
	}
	const vaultInitialized = await LegacyVault.isInitialized();
	if (!vaultInitialized) {
		return (statusCache = 'ready');
	}
	const isMigrationDone = await getFromLocalStorage<boolean>(migrationDoneStorageKey);
	if (isMigrationDone) {
		return (statusCache = 'ready');
	}
	return (statusCache = 'required');
}

export function clearStatus() {
	statusCache = null;
}

async function makeMnemonicAccounts(password: string, vault: LegacyVault) {
	const currentMnemonicIndex = (await getFromLocalStorage<number>('last_account_index', 0)) || 0;
	const mnemonicSource = await MnemonicAccountSource.createNew({
		password,
		entropyInput: vault.entropy,
	});
	const mnemonicAccounts = [];
	for (let i = 0; i <= currentMnemonicIndex; i++) {
		const derivationPath = makeDerivationPath(i);
		const keyPair = deriveKeypairFromSeed(vault.mnemonicSeedHex, derivationPath);
		mnemonicAccounts.push(
			MnemonicAccount.createNew({ keyPair, derivationPath, sourceID: mnemonicSource.id }),
		);
	}
	return { mnemonicSource, mnemonicAccounts };
}

async function makeImportedAccounts(password: string, vault: LegacyVault) {
	return Promise.all(
		vault.importedKeypairs.map((keyPair) =>
			ImportedAccount.createNew({ password, keyPair: keyPair.getSecretKey() }),
		),
	);
}

type LegacySerializedLedgerAccount = {
	type: 'LEDGER';
	address: string;
	derivationPath: string;
	publicKey: string | null;
};

async function getSavedLedgerAccounts() {
	const ledgerAccounts = await getFromLocalStorage<LegacySerializedLedgerAccount[]>(
		'imported_ledger_accounts',
		[],
	);
	return ledgerAccounts || [];
}

async function makeLedgerAccounts(password: string) {
	const ledgerAccounts = await getSavedLedgerAccounts();
	return Promise.all(
		ledgerAccounts.map(({ address, derivationPath, publicKey }) =>
			LedgerAccount.createNew({ address, derivationPath, password, publicKey }),
		),
	);
}

function withID<T extends Omit<SerializedAccount, 'id'>>(anAccount: T) {
	return {
		...anAccount,
		id: makeUniqueKey(),
	};
}

export async function doMigration(password: string) {
	const legacyVault = await LegacyVault.fromLegacyStorage(password);
	const currentStatus = await getStatus();
	if (currentStatus === 'required') {
		statusCache = 'inProgress';
		try {
			const db = await getDB();
			const currentActiveAccountAddress = await getActiveAccountAddress();
			const { mnemonicAccounts, mnemonicSource } = await makeMnemonicAccounts(
				password,
				legacyVault,
			);
			const importedAccounts = await makeImportedAccounts(password, legacyVault);
			const ledgerAccounts = await makeLedgerAccounts(password);
			await db.transaction('rw', db.accounts, db.accountSources, async () => {
				await MnemonicAccountSource.save(mnemonicSource, { skipBackup: true, skipEventEmit: true });
				await db.accounts.bulkPut(mnemonicAccounts.map(withID));
				await db.accounts.bulkPut(importedAccounts.map(withID));
				await db.accounts.bulkPut(ledgerAccounts.map(withID));
				if (currentActiveAccountAddress) {
					const accountToSetSelected = await db.accounts.get({
						address: currentActiveAccountAddress,
					});
					if (accountToSetSelected) {
						await db.accounts
							.where('id')
							.notEqual(accountToSetSelected.id)
							.modify({ selected: false });
						await db.accounts.update(accountToSetSelected.id, { selected: true });
					}
				}
				await Dexie.waitFor(setToLocalStorage(migrationDoneStorageKey, true));
			});
			statusCache = 'ready';
			backupDB();
			accountSourcesEvents.emit('accountSourcesChanged');
			accountsEvents.emit('accountsChanged');
		} catch (e) {
			statusCache = 'required';
			throw e;
		}
	}
}
