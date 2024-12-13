// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CommonOutputObjectWithUc } from '@iota/core';
import { IotaObjectData } from '@iota/iota-sdk/client';

export type StardustMigrationGroupedObjects = {
    migratable: IotaObjectData[];
    unmigratable: IotaObjectData[];
};

export function groupStardustObjectsByMigrationStatus(
    stardustOutputObjects: IotaObjectData[],
    epochTimestamp: number,
    address: string,
): StardustMigrationGroupedObjects {
    const migratable: IotaObjectData[] = [];
    const unmigratable: IotaObjectData[] = [];

    const epochUnix = epochTimestamp / 1000;

    for (const outputObject of stardustOutputObjects) {
        const outputObjectFields = extractOutputFields(outputObject);

        if (outputObjectFields.expiration_uc) {
            const unlockableAddress =
                outputObjectFields.expiration_uc.fields.unix_time <= epochUnix
                    ? outputObjectFields.expiration_uc.fields.return_address
                    : outputObjectFields.expiration_uc.fields.owner;

            if (unlockableAddress !== address) {
                unmigratable.push(outputObject);
                continue;
            }
        }
        if (
            outputObjectFields.timelock_uc &&
            outputObjectFields.timelock_uc.fields.unix_time > epochUnix
        ) {
            unmigratable.push(outputObject);
            continue;
        }

        migratable.push(outputObject);
    }

    return { migratable, unmigratable };
}

interface MigratableObjectsData {
    totalNativeTokens: number;
    totalVisualAssets: number;
    accumulatedIotaAmount: number;
}

export function summarizeMigratableObjectValues({
    migratableBasicOutputs,
    migratableNftOutputs,
    address,
}: {
    migratableBasicOutputs: IotaObjectData[];
    migratableNftOutputs: IotaObjectData[];
    address: string;
}): MigratableObjectsData {
    let totalNativeTokens = 0;
    let totalIotaAmount = 0;

    const totalVisualAssets = migratableNftOutputs.length;
    const outputObjects = [...migratableBasicOutputs, ...migratableNftOutputs];

    for (const output of outputObjects) {
        const outputObjectFields = extractOutputFields(output);

        totalIotaAmount += parseInt(outputObjectFields.balance);
        totalNativeTokens += parseInt(outputObjectFields.native_tokens.fields.size);
        totalIotaAmount += extractStorageDepositReturnAmount(outputObjectFields, address) || 0;
    }

    return { totalNativeTokens, totalVisualAssets, accumulatedIotaAmount: totalIotaAmount };
}

function extractStorageDepositReturnAmount(
    { storage_deposit_return_uc }: CommonOutputObjectWithUc,
    address: string,
): number | null {
    if (
        storage_deposit_return_uc?.fields &&
        storage_deposit_return_uc?.fields.return_address === address
    ) {
        return parseInt(storage_deposit_return_uc?.fields.return_amount);
    }
    return null;
}

function extractOutputFields(outputObject: IotaObjectData): CommonOutputObjectWithUc {
    return (
        outputObject.content as unknown as {
            fields: CommonOutputObjectWithUc;
        }
    ).fields;
}
