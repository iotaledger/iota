// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import Dexie, { type Table } from 'dexie';
import { exportDB, importDB } from 'dexie-export-import';

import { type AccountSourceSerialized } from './account-sources/accountSource';
import { type SerializedAccount } from './accounts/account';
import { captureException } from './sentry';
import { getFromLocalStorage, setToLocalStorage } from './storageUtils';

const DB_NAME = 'IotaWallet DB';
const DB_LOCAL_STORAGE_BACKUP_KEY = 'indexed-db-backup';

export const SETTINGS_KEYS = {
    isPopulated: 'isPopulated',
    autoLockMinutes: 'auto-lock-minutes',
};

class DB extends Dexie {
    accountSources!: Table<AccountSourceSerialized, string>;
    accounts!: Table<SerializedAccount, string>;
    settings!: Table<{ value: boolean | number | null; setting: string }, string>;

    constructor() {
        super(DB_NAME);
        this.version(1).stores({
            accountSources: 'id, type',
            accounts: 'id, type, address, sourceID',
            settings: 'setting',
        });
    }
}

async function init() {
    const db = new DB();
    const isPopulated = !!(await db.settings.get(SETTINGS_KEYS.isPopulated))?.value;
    if (!isPopulated) {
        try {
            const backup = await getFromLocalStorage<string>(DB_LOCAL_STORAGE_BACKUP_KEY);
            if (backup) {
                captureException(
                    new Error('IndexedDB is empty, attempting to restore from backup'),
                    {
                        extra: { backupSize: backup.length },
                    },
                );
                await db.delete();
                (await importDB(new Blob([backup], { type: 'application/json' }))).close();
                await db.open();
            }
            await db.settings.put({ setting: SETTINGS_KEYS.isPopulated, value: true });
        } catch (e) {
            captureException(e);
        }
    }
    if (!db.isOpen()) {
        await db.open();
    }
    return db;
}
let initPromise: ReturnType<typeof init> | null = null;
export const getDB = () => {
    if (!initPromise) {
        initPromise = init();
    }
    return initPromise;
};

export async function backupDB() {
    try {
        const backup = await (await exportDB(await getDB())).text();
        await setToLocalStorage(DB_LOCAL_STORAGE_BACKUP_KEY, backup);
    } catch (e) {
        captureException(e);
    }
}
