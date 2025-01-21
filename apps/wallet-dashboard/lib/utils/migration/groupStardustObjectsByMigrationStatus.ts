// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CommonOutputObjectWithUc, MILLISECONDS_PER_SECOND } from '@iota/core';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { filterMigrationObjects } from './filterMigrationObjectDetails';
import { CommonMigrationObjectType, StardustOutputDetailsFilter } from '@/lib/enums';
import { ResolvedObjectTypes } from '@/lib/types';

export type StardustMigrationGroupedObjects = {
    migratable: IotaObjectData[];
    timelocked: IotaObjectData[];
};

export function groupStardustObjectsByMigrationStatus(
    stardustOutputObjects: IotaObjectData[],
    epochTimestampMs: number,
    address: string,
): StardustMigrationGroupedObjects {
    const migratable: IotaObjectData[] = [];
    const timelocked: IotaObjectData[] = [];

    const epochUnix = epochTimestampMs / MILLISECONDS_PER_SECOND;

    for (const outputObject of stardustOutputObjects) {
        const outputObjectFields = extractOutputFields(outputObject);

        if (
            outputObjectFields.timelock_uc &&
            outputObjectFields.timelock_uc.fields.unix_time > epochUnix
        ) {
            timelocked.push(outputObject);
            continue;
        }

        if (outputObjectFields.expiration_uc) {
            const unlockableAddress =
                outputObjectFields.expiration_uc.fields.unix_time <= epochUnix
                    ? outputObjectFields.expiration_uc.fields.return_address
                    : outputObjectFields.expiration_uc.fields.owner;

            if (unlockableAddress !== address) {
                continue;
            }
        }

        migratable.push(outputObject);
    }

    return { migratable, timelocked };
}

interface StardustObjectsTotals {
    totalNativeTokens: number;
    totalVisualAssets: number;
    totalIotaAmount: bigint;
    totalNotOwnedStorageDepositReturnAmount: bigint;
}

interface StardustObjectsTotalsParams {
    basicOutputs: IotaObjectData[] | undefined;
    nftOutputs: IotaObjectData[] | undefined;
    address: string;
    resolvedObjects?: ResolvedObjectTypes[];
}

export function getStardustObjectsTotals({
    basicOutputs = [],
    nftOutputs = [],
    address,
    resolvedObjects,
}: StardustObjectsTotalsParams): StardustObjectsTotals {
    let totalNativeTokens = 0;
    let totalIotaAmount: bigint = 0n;
    let totalNotOwnedStorageDepositReturnAmount: bigint = 0n;
    let filteredObjects: ResolvedObjectTypes[] = [];

    const totalVisualAssets = nftOutputs.length;
    const outputObjects = [...basicOutputs, ...nftOutputs];

    if (resolvedObjects) {
        filteredObjects = filterMigrationObjects(
            resolvedObjects,
            StardustOutputDetailsFilter.NativeTokens,
        );
    }
    totalNativeTokens = getUniqueNativeTokensByCoinType(filteredObjects);

    for (const output of outputObjects) {
        const outputObjectFields = extractOutputFields(output);

        totalIotaAmount += BigInt(outputObjectFields.balance);
        totalIotaAmount +=
            extractOwnedStorageDepositReturnAmount(outputObjectFields, address) || 0n;
        totalNotOwnedStorageDepositReturnAmount +=
            extractNotOwnedStorageDepositReturnAmount(outputObjectFields, address) || 0n;
    }

    return {
        totalNativeTokens,
        totalVisualAssets,
        totalIotaAmount,
        totalNotOwnedStorageDepositReturnAmount,
    };
}

export function extractOwnedStorageDepositReturnAmount(
    { storage_deposit_return_uc }: CommonOutputObjectWithUc,
    address: string,
): bigint | null {
    if (
        storage_deposit_return_uc?.fields &&
        storage_deposit_return_uc?.fields.return_address === address
    ) {
        return BigInt(storage_deposit_return_uc?.fields.return_amount);
    }
    return null;
}

export function extractOutputFields(outputObject: IotaObjectData): CommonOutputObjectWithUc {
    return (
        outputObject.content as unknown as {
            fields: CommonOutputObjectWithUc;
        }
    ).fields;
}

export function extractNotOwnedStorageDepositReturnAmount(
    { storage_deposit_return_uc }: CommonOutputObjectWithUc,
    address: string,
): bigint | null {
    if (
        storage_deposit_return_uc?.fields &&
        storage_deposit_return_uc?.fields.return_address !== address
    ) {
        return BigInt(storage_deposit_return_uc?.fields.return_amount);
    }
    return null;
}

export function getUniqueNativeTokensByCoinType(objects: ResolvedObjectTypes[]): number {
    return new Set(
        objects
            .filter((obj) => obj.commonObjectType === CommonMigrationObjectType.NativeToken)
            .map((obj) => obj.coinType),
    ).size;
}
