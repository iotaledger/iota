// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CommonOutputObjectWithUc, MILLISECONDS_PER_SECOND } from '@iota/core';
import { IotaObjectData } from '@iota/iota-sdk/client';

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
        const outputObjectFields = extractMigrationOutputFields(outputObject);

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

interface MigratableObjectsData {
    totalNativeTokens: number;
    totalVisualAssets: number;
    totalIotaAmount: bigint;
    totalNotOwnedStorageDepositReturnAmount: bigint;
}

interface SummarizeMigrationObjectParams {
    basicOutputs: IotaObjectData[] | undefined;
    nftOutputs: IotaObjectData[] | undefined;
    address: string;
}

export function summarizeMigratableObjectValues({
    basicOutputs = [],
    nftOutputs = [],
    address,
}: SummarizeMigrationObjectParams): MigratableObjectsData {
    let totalNativeTokens = 0;
    let totalIotaAmount: bigint = 0n;
    let totalNotOwnedStorageDepositReturnAmount: bigint = 0n;

    const totalVisualAssets = nftOutputs.length;
    const outputObjects = [...basicOutputs, ...nftOutputs];

    for (const output of outputObjects) {
        const outputObjectFields = extractMigrationOutputFields(output);

        totalIotaAmount += BigInt(outputObjectFields.balance);
        totalNativeTokens += parseInt(outputObjectFields.native_tokens.fields.size);
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

interface UnmmigratableObjectsData {
    totalTimelockedObjects: number;
}

export function summarizeTimelockedObjectValues({
    basicOutputs = [],
    nftOutputs = [],
}: Omit<SummarizeMigrationObjectParams, 'address'>): UnmmigratableObjectsData {
    const basicObjects = basicOutputs.length;
    const nftObjects = nftOutputs.length;
    let nativeTokens = 0;

    for (const output of [...basicOutputs, ...nftOutputs]) {
        const outputObjectFields = extractMigrationOutputFields(output);

        nativeTokens += parseInt(outputObjectFields.native_tokens.fields.size);
    }

    const totalTimelockedObjects = basicObjects + nativeTokens + nftObjects;

    return { totalTimelockedObjects };
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

export function extractMigrationOutputFields(
    outputObject: IotaObjectData,
): CommonOutputObjectWithUc {
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
