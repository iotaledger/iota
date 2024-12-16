// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CommonMigrationObjectType } from '@/lib/enums';
import {
    ResolvedBasicObject,
    ResolvedNativeToken,
    ResolvedNftObject,
    ResolvedObjectTypes,
    UnlockConditionTimestamp,
} from '@/lib/types';
import {
    extractObjectTypeStruct,
    getNativeTokensFromBag,
    STARDUST_BASIC_OUTPUT_TYPE,
    STARDUST_NFT_OUTPUT_TYPE,
} from '@iota/core';
import { extractMigrationOutputFields, extractStorageDepositReturnAmount } from '.';
import { IotaClient, IotaObjectData } from '@iota/iota-sdk/client';
import { MIGRATION_OBJECT_WITHOUT_UC_KEY } from '@/lib/constants';

export async function groupMigrationObjectsByUnlockCondition(
    objectsData: IotaObjectData[],
    client: IotaClient,
    currentAddress: string = '',
    isTimelockUnlockCondition: boolean = false,
): Promise<ResolvedObjectTypes[]> {
    const flatObjects: ResolvedObjectTypes[] = [];
    const basicObjectMap: Map<string, ResolvedBasicObject> = new Map();
    const nativeTokenMap: Map<string, Map<string, ResolvedNativeToken>> = new Map();

    const PROMISE_CHUNK_SIZE = 100;

    // Get output data in chunks of 100
    for (let i = 0; i < objectsData.length; i += PROMISE_CHUNK_SIZE) {
        const chunk = objectsData.slice(i, i + PROMISE_CHUNK_SIZE);

        const promises = chunk.map(async (object) => {
            const objectFields = extractMigrationOutputFields(object);

            let groupKey: string | undefined;
            if (isTimelockUnlockCondition) {
                const timestamp = objectFields.timelock_uc?.fields.unix_time.toString();
                groupKey = timestamp;
            } else {
                const timestamp = objectFields.expiration_uc?.fields.unix_time.toString();
                // Timestamp can be undefined if the object was timelocked and is now unlocked
                // and it doesn't have an expiration unlock condition
                groupKey = timestamp ?? MIGRATION_OBJECT_WITHOUT_UC_KEY;
            }

            if (!groupKey) {
                return;
            }

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
            const objectNativeTokens = await extractNativeTokensFromObject(
                object,
                client,
                groupKey,
            );

            for (const token of objectNativeTokens) {
                const existing = tokenGroup.get(token.name);

                if (existing) {
                    existing.balance += token.balance;
                } else {
                    tokenGroup.set(token.name, token);
                    flatObjects.push(token);
                }
            }
        });

        // Wait for all promises in the chunk to resolve
        await Promise.all(promises);
    }

    const parseTimestamp = (timestamp: string) =>
        timestamp === MIGRATION_OBJECT_WITHOUT_UC_KEY ? 0 : parseInt(timestamp);
    flatObjects.sort((a, b) => {
        const timestampA = parseTimestamp(a.unlockConditionTimestamp);
        const timestampB = parseTimestamp(b.unlockConditionTimestamp);
        return timestampA - timestampB;
    });
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
