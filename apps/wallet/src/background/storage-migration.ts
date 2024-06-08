// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getFromLocalStorage } from './storage-utils';

export type Status = 'required' | 'inProgress' | 'ready';

const migrationDoneStorageKey = 'storage-migration-done';

let statusCache: Status | null = null;

export async function getStatus() {
    // placeholde for migration status, always returns ready
    if (statusCache) {
        return statusCache;
    }
    const isMigrationDone = await getFromLocalStorage<boolean>(migrationDoneStorageKey);
    if (isMigrationDone) {
        return (statusCache = 'ready');
    }
    return (statusCache = 'ready');
}

export function clearStatus() {
    statusCache = null;
}

export async function doMigration(password: string) {
    // placeholder for migration logic
}
