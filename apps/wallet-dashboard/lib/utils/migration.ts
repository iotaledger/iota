// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CommonOutputObjectWithUc } from '@iota/core';
import { IotaObjectData } from '@iota/iota-sdk/client';
import {
    extractObjectTypeStruct,
    getNativeTokensFromBag,
    STARDUST_BASIC_OUTPUT_TYPE,
    STARDUST_NFT_OUTPUT_TYPE,
} from '@iota/core';
import { IotaClient } from '@iota/iota-sdk/client';
import { CommonMigrationObjectType } from '../enums';
import {
    UnlockConditionTimestamp,
    ResolvedNativeToken,
    ResolvedNftObject,
    ResolvedObjectTypes,
    ResolvedBasicObject,
} from '../types';
import { MIGRATION_OBJECT_WITHOUT_UC_KEY } from '../constants';

export type StardustMigrationGroupedObjects = {
    migratable: IotaObjectData[];
    unmigratable: IotaObjectData[];
};

export function groupStardustObjectsByMigrationStatus(
    stardustOutputObjects: IotaObjectData[],
    epochTimestampMs: number,
    address: string,
): StardustMigrationGroupedObjects {
    const migratable: IotaObjectData[] = [];
    const unmigratable: IotaObjectData[] = [];

    const unixEpoch = epochTimestampMs / 1000;

    for (const outputObject of stardustOutputObjects) {
        const outputObjectFields = extractMigrationOutputFields(outputObject);

        if (outputObjectFields.expiration_uc) {
            const unlockableAddress =
                outputObjectFields.expiration_uc.fields.unix_time <= unixEpoch
                    ? outputObjectFields.expiration_uc.fields.return_address
                    : outputObjectFields.expiration_uc.fields.owner;

            if (unlockableAddress !== address) {
                unmigratable.push(outputObject);
                continue;
            }
        }

        if (
            outputObjectFields.timelock_uc &&
            outputObjectFields.timelock_uc.fields.unix_time > unixEpoch
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
    totalIotaAmount: bigint;
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

    const totalVisualAssets = nftOutputs.length;
    const outputObjects = [...basicOutputs, ...nftOutputs];

    for (const output of outputObjects) {
        const outputObjectFields = extractMigrationOutputFields(output);

        totalIotaAmount += BigInt(outputObjectFields.balance);
        totalNativeTokens += parseInt(outputObjectFields.native_tokens.fields.size);
        totalIotaAmount += extractStorageDepositReturnAmount(outputObjectFields, address) || 0n;
    }

    return { totalNativeTokens, totalVisualAssets, totalIotaAmount };
}

interface UnmmigratableObjectsData {
    totalUnmigratableObjects: number;
}

export function summarizeUnmigratableObjectValues({
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

    const totalUnmigratableObjects = basicObjects + nativeTokens + nftObjects;

    return { totalUnmigratableObjects };
}

export function extractStorageDepositReturnAmount(
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
export async function groupMigrationObjectsByUnlockCondition(
    objectsData: IotaObjectData[],
    client: IotaClient,
    currentAddress: string = '',
    isTimelockUnlockCondition: boolean = false,
): Promise<ResolvedObjectTypes[]> {
    const flatObjects: ResolvedObjectTypes[] = [];
    const basicObjectMap: Map<string, ResolvedBasicObject> = new Map();
    const nativeTokenMap: Map<string, Map<string, ResolvedNativeToken>> = new Map();

    for (const object of objectsData) {
        const objectFields = extractMigrationOutputFields(object);
        const groupKey = getGroupingObjectsKey(isTimelockUnlockCondition, objectFields);

        if (object.type === STARDUST_BASIC_OUTPUT_TYPE) {
            const existing = basicObjectMap.get(groupKey);
            const gasReturn = extractStorageDepositReturnAmount(objectFields, currentAddress);
            const newBalance =
                (existing ? existing.balance : 0n) +
                BigInt(objectFields.balance) +
                (gasReturn ?? 0n);

            if (existing) {
                existing.balance = newBalance;
            } else {
                const newBasicObject: ResolvedBasicObject = {
                    balance: newBalance,
                    unlockConditionTimestamp: groupKey,
                    type: object.type,
                    commonObjectType: CommonMigrationObjectType.Basic,
                    output: object,
                    uniqueId: objectFields.id.id,
                };
                basicObjectMap.set(groupKey, newBasicObject);
                flatObjects.push(newBasicObject);
            }
        } else if (object.type === STARDUST_NFT_OUTPUT_TYPE) {
            const nftDetails = await getNftDetails(object, groupKey, client);
            flatObjects.push(...nftDetails);
        }

        if (!nativeTokenMap.has(groupKey)) {
            nativeTokenMap.set(groupKey, new Map());
        }

        const tokenGroup = nativeTokenMap.get(groupKey)!;
        const objectNativeTokens = await extractNativeTokensFromObject(object, client, groupKey);

        for (const token of objectNativeTokens) {
            const existing = tokenGroup.get(token.name);

            if (existing) {
                existing.balance += token.balance;
            } else {
                tokenGroup.set(token.name, token);
                flatObjects.push(token);
            }
        }
    }

    return flatObjects;
}

async function getNftDetails(
    object: IotaObjectData,
    expirationKey: UnlockConditionTimestamp,
    client: IotaClient,
): Promise<ResolvedNftObject[]> {
    const objectFields = extractMigrationOutputFields(object);
    const nftOutputDynamicFields = await client.getDynamicFields({
        parentId: objectFields.id.id,
    });

    const nftDetails: ResolvedNftObject[] = [];
    for (const nft of nftOutputDynamicFields.data) {
        const nftObject = await client.getObject({
            id: nft.objectId,
            options: { showDisplay: true },
        });

        if (!nftObject?.data?.display?.data) {
            continue;
        }

        nftDetails.push({
            balance: BigInt(objectFields.balance),
            name: nftObject.data.display.data.name ?? '',
            image_url: nftObject.data.display.data.image_url ?? '',
            commonObjectType: CommonMigrationObjectType.Nft,
            unlockConditionTimestamp: expirationKey,
            output: object,
            uniqueId: nftObject.data.objectId,
        });
    }

    return nftDetails;
}

async function extractNativeTokensFromObject(
    object: IotaObjectData,
    client: IotaClient,
    expirationKey: UnlockConditionTimestamp,
): Promise<ResolvedNativeToken[]> {
    const fields = extractMigrationOutputFields(object);
    const bagId = fields.native_tokens.fields.id.id;
    const bagSize = Number(fields.native_tokens.fields.size);

    const nativeTokens = bagSize > 0 ? await getNativeTokensFromBag(bagId, client) : [];
    const result: ResolvedNativeToken[] = [];

    for (const nativeToken of nativeTokens) {
        const nativeTokenParentId = fields.native_tokens.fields.id.id;
        const objectDynamic = await client.getDynamicFieldObject({
            parentId: nativeTokenParentId,
            name: nativeToken.name,
        });

        if (objectDynamic?.data?.content && 'fields' in objectDynamic.data.content) {
            const nativeTokenFields = objectDynamic.data.content.fields as {
                name: string;
                value: string;
                id: { id: string };
            };
            const tokenStruct = extractObjectTypeStruct(nativeTokenFields.name);
            const tokenName = tokenStruct[2];
            const balance = BigInt(nativeTokenFields.value);

            result.push({
                name: tokenName,
                balance,
                coinType: nativeTokenFields.name,
                unlockConditionTimestamp: expirationKey,
                commonObjectType: CommonMigrationObjectType.NativeToken,
                output: object,
                uniqueId: nativeTokenFields.id.id,
            });
        }
    }

    return result;
}

function getGroupingObjectsKey(
    isTimelockUnlockCondition: boolean,
    objectFields: CommonOutputObjectWithUc,
): string {
    if (!isTimelockUnlockCondition) {
        return (
            objectFields.expiration_uc?.fields.unix_time.toString() ??
            MIGRATION_OBJECT_WITHOUT_UC_KEY
        );
    } else {
        return (
            objectFields.timelock_uc?.fields.unix_time.toString() ?? MIGRATION_OBJECT_WITHOUT_UC_KEY
        );
    }
}
