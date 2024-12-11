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
    ExpirationMigrationKey,
    ResolvedNativeToken,
    ResolvedNftObject,
    ResolvedObjectsGrouped,
} from '../types';
import { MIGRATION_OBJECT_WITHOUT_EXPIRATION_KEY } from '../constants';

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
        const outputObjectFields = extractMigrationOutputFields(outputObject);

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
    totalIotaAmount: number;
}

interface SummarizeMigratableObjectValuesParams {
    migratableBasicOutputs: IotaObjectData[];
    migratableNftOutputs: IotaObjectData[];
    address: string;
}

export function summarizeMigratableObjectValues({
    migratableBasicOutputs,
    migratableNftOutputs,
    address,
}: SummarizeMigratableObjectValuesParams): MigratableObjectsData {
    let totalNativeTokens = 0;
    let totalIotaAmount = 0;

    const totalVisualAssets = migratableNftOutputs.length;
    const outputObjects = [...migratableBasicOutputs, ...migratableNftOutputs];

    for (const output of outputObjects) {
        const outputObjectFields = extractMigrationOutputFields(output);

        totalIotaAmount += parseInt(outputObjectFields.balance);
        totalNativeTokens += parseInt(outputObjectFields.native_tokens.fields.size);
        totalIotaAmount += extractStorageDepositReturnAmount(outputObjectFields, address) || 0;
    }

    return { totalNativeTokens, totalVisualAssets, totalIotaAmount };
}

export function extractStorageDepositReturnAmount(
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

export function extractMigrationOutputFields(
    outputObject: IotaObjectData,
): CommonOutputObjectWithUc {
    return (
        outputObject.content as unknown as {
            fields: CommonOutputObjectWithUc;
        }
    ).fields;
}
export async function groupMigrationObjectsByExpirationDate(
    objectsData: IotaObjectData[],
    client: IotaClient,
    currentAddress?: string,
) {
    const nftObjects: ResolvedObjectsGrouped['nftObjects'] = {};
    const basicObjects: ResolvedObjectsGrouped['basicObjects'] = {};
    const nativeTokens: ResolvedObjectsGrouped['nativeTokens'] = {};

    for (const object of objectsData) {
        const objectFields = extractMigrationOutputFields(object);
        let expirationKey: ExpirationMigrationKey;

        if (!objectFields.expiration_uc) {
            expirationKey = MIGRATION_OBJECT_WITHOUT_EXPIRATION_KEY;
        } else {
            expirationKey = objectFields.expiration_uc.fields.unix_time.toString();
        }

        if (object.type === STARDUST_BASIC_OUTPUT_TYPE) {
            // Merge balances if already present
            const existing = basicObjects[expirationKey];
            const gasReturn = extractStorageDepositReturnAmount(objectFields, currentAddress ?? '');
            const newBalance =
                (existing ? existing.balance : 0) + Number(objectFields.balance) + (gasReturn ?? 0);
            basicObjects[expirationKey] = {
                balance: newBalance,
                expirationKey: expirationKey,
                type: object.type,
                commonObjectType: CommonMigrationObjectType.Basic,
                output: object,
                uniqueId: objectFields.id.id,
            };
        } else if (object.type === STARDUST_NFT_OUTPUT_TYPE) {
            const nftDetails = await getNftDetails(object, expirationKey, client);
            if (nftObjects[expirationKey]) {
                nftObjects[expirationKey].push(...nftDetails);
            } else {
                nftObjects[expirationKey] = nftDetails;
            }
        }

        const objectNativeTokens = await extractNativeTokensFromObject(
            object,
            client,
            expirationKey,
        );
        mergeNativeTokens(nativeTokens, objectNativeTokens, expirationKey);
    }

    return { nftObjects, basicObjects, nativeTokens };
}

async function getNftDetails(
    object: IotaObjectData,
    expirationKey: ExpirationMigrationKey,
    client: IotaClient,
) {
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
            balance: Number(objectFields.balance),
            name: nftObject.data.display.data.name ?? '',
            image_url: nftObject.data.display.data.image_url ?? '',
            commonObjectType: CommonMigrationObjectType.Nft,
            expirationKey: expirationKey,
            output: object,
            uniqueId: nftObject.data.objectId,
        });
    }

    return nftDetails;
}

async function extractNativeTokensFromObject(
    object: IotaObjectData,
    client: IotaClient,
    expirationKey: ExpirationMigrationKey,
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
                id: { id: string };
                name: string;
                value: string;
            };

            const tokenStruct = extractObjectTypeStruct(nativeTokenFields.name);
            const tokenName = tokenStruct[2];
            const balance = Number(nativeTokenFields.value);

            result.push({
                name: tokenName,
                balance,
                coinType: nativeTokenFields.name,
                expirationKey: expirationKey,
                commonObjectType: CommonMigrationObjectType.NativeToken,
                output: object,
                uniqueId: nativeTokenFields.id.id,
            });
        }
    }

    return result;
}

function mergeNativeTokens(
    nativeTokens: ResolvedObjectsGrouped['nativeTokens'],
    tokensToAdd: ResolvedNativeToken[],
    expirationKey: ExpirationMigrationKey,
) {
    if (!nativeTokens[expirationKey]) {
        nativeTokens[expirationKey] = {};
    }

    for (const token of tokensToAdd) {
        const existing = nativeTokens[expirationKey][token.name];
        const newBalance = (existing ? existing.balance : 0) + token.balance;
        nativeTokens[expirationKey][token.name] = { ...token, balance: newBalance };
    }
}
