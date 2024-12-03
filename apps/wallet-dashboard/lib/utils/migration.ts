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

export function getTotalTimelockedAmount(stardustOutputObjects: IotaObjectData[]): number {
    let totalTimelockedAmount = 0;

    for (const outputObject of stardustOutputObjects) {
        const outputObjectFields = extractOutputFields(outputObject);

        totalTimelockedAmount += parseInt(outputObjectFields.balance);

        totalTimelockedAmount += parseInt(outputObjectFields.balance);
        const depositReturnAmount = calculateStorageDepositReturn(outputObjectFields);

        if (depositReturnAmount) {
            totalTimelockedAmount += depositReturnAmount;
        }
    }

    return totalTimelockedAmount;
}

interface MigratableObjectsData {
    totalNativeTokens: number;
    totalVisualAssets: number;
    accumulatedIotaAmount: number;
}

export function summarizeMigrationValues({
    basicOutputObjects,
    nftOutputObjects,
    epochUnix,
    address,
}: {
    basicOutputObjects: IotaObjectData[];
    nftOutputObjects: IotaObjectData[];
    epochUnix: number;
    address: string;
}): MigratableObjectsData {
    let totalNativeTokens = 0;
    let totalIotaAmount = 0;

    const totalVisualAssets = nftOutputObjects.length;
    const outputObjects = [...basicOutputObjects, ...nftOutputObjects];

    for (const output of outputObjects) {
        const outputObjectFields = extractOutputFields(output);

        totalIotaAmount += parseInt(outputObjectFields.balance);
        totalNativeTokens += parseInt(outputObjectFields.native_tokens.fields.size);

        if (outputObjectFields.expiration_uc) {
            const unlockableAddress = getUnlockableAddressFromUnlockConditions(
                outputObjectFields.expiration_uc,
                epochUnix,
            );

            // Only add the return amount if the unlockable address is the current address
            if (unlockableAddress === address) {
                const depositReturnAmount = calculateStorageDepositReturn(outputObjectFields) ?? 0;
                totalIotaAmount += depositReturnAmount;
            }
        }
    }

    return { totalNativeTokens, totalVisualAssets, accumulatedIotaAmount: totalIotaAmount };
}

function calculateStorageDepositReturn({
    storage_deposit_return_uc,
}: CommonOutputObjectWithUc): number | undefined {
    if (
        storage_deposit_return_uc &&
        'fields' in storage_deposit_return_uc &&
        'return_amount' in storage_deposit_return_uc.fields
    ) {
        return parseInt(storage_deposit_return_uc?.fields.return_amount);
    }
}

function extractOutputFields(outputObject: IotaObjectData): CommonOutputObjectWithUc {
    return (
        outputObject.content as unknown as {
            fields: CommonOutputObjectWithUc;
        }
    ).fields;
}

function getUnlockableAddressFromUnlockConditions(
    unlockCondition: Exclude<CommonOutputObjectWithUc['expiration_uc'], null | undefined>,
    epochUnix: number,
): string {
    return unlockCondition.fields.unix_time <= epochUnix
        ? unlockCondition.fields.return_address
        : unlockCondition.fields.owner;
}
