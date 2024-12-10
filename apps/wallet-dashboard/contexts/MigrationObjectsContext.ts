// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaObjectData } from '@iota/iota-sdk/client';
import { createContext, useContext } from 'react';

interface MigrationObjectsContext {
    migratableBasicOutputs: IotaObjectData[];
    unmigratableBasicOutputs: IotaObjectData[];
    migratableNftOutputs: IotaObjectData[];
    unmigratableNftOutputs: IotaObjectData[];
}

export const MigrationObjectsContext = createContext<MigrationObjectsContext | undefined>(
    undefined,
);

export function useMigrationObjectsContext(): MigrationObjectsContext {
    const migrationObjects = useContext(MigrationObjectsContext);

    if (migrationObjects === undefined) {
        throw new Error(
            'useMigrationObjectsContext must be used within a MigrationObjectsContextProvider',
        );
    }

    return migrationObjects;
}
