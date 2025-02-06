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
    MILLISECONDS_PER_SECOND,
    STARDUST_BASIC_OUTPUT_TYPE,
    STARDUST_NFT_OUTPUT_TYPE,
} from '@iota/core';
import { extractOutputFields, extractOwnedStorageDepositReturnAmount } from '.';
import { IotaClient, IotaObjectData } from '@iota/iota-sdk/client';
import { MIGRATION_OBJECT_WITHOUT_UC_KEY } from '@/lib/constants';
import { normalizeStructTag } from '@iota/iota-sdk/utils';

export async function groupMigrationObjectsByUnlockCondition(
    objectsData: IotaObjectData[],
    client: IotaClient,
    currentAddress: string = '',
    groupByTimelockUC: boolean = false, // If true, group by timelock unlock condition, else group by expiration unlock condition
    currentEpochStartMs?: number,
): Promise<ResolvedObjectTypes[]> {
    const flatObjects: ResolvedObjectTypes[] = [];
    const basicObjectMap: Map<string, ResolvedBasicObject> = new Map();
    const nativeTokenMap: Map<string, Map<string, ResolvedNativeToken>> = new Map();

    const PROMISE_CHUNK_SIZE = 100;

    // Get output data in chunks of 100
    for (let i = 0; i < objectsData.length; i += PROMISE_CHUNK_SIZE) {
        const chunk = objectsData.slice(i, i + PROMISE_CHUNK_SIZE);

        const promises = chunk.map(async (object) => {
            const objectFields = extractOutputFields(object);

            let groupKey: string | undefined;
            if (groupByTimelockUC) {
                const timestamp = objectFields.timelock_uc?.fields.unix_time.toString();
                groupKey = timestamp;
            } else {
                const timestamp = objectFields.expiration_uc?.fields.unix_time.toString();
                // Timestamp can be undefined if the object was timelocked and is now unlocked
                // and it doesn't have an expiration unlock condition
                groupKey =
                    timestamp &&
                    currentEpochStartMs !== undefined &&
                    Number(timestamp) >= currentEpochStartMs / MILLISECONDS_PER_SECOND
                        ? timestamp
                        : MIGRATION_OBJECT_WITHOUT_UC_KEY;
            }

            if (!groupKey) {
                return;
            }

            const existingBasicObject = basicObjectMap.get(groupKey);
            const gasReturn = extractOwnedStorageDepositReturnAmount(objectFields, currentAddress);
            const newBalance =
                (existingBasicObject ? existingBasicObject.balance : 0n) +
                BigInt(objectFields.balance) +
                (gasReturn ?? 0n);

            if (existingBasicObject) {
                existingBasicObject.balance = newBalance;
            } else {
                const newBasicObject: ResolvedBasicObject = {
                    balance: newBalance,
                    unlockConditionTimestamp: groupKey,
                    type: STARDUST_BASIC_OUTPUT_TYPE,
                    commonObjectType: CommonMigrationObjectType.Basic,
                    output: object,
                    uniqueId: objectFields.id.id,
                };
                basicObjectMap.set(groupKey, newBasicObject);
                flatObjects.push(newBasicObject);
            }

            if (object.type) {
                const normalizedObjectStruct = normalizeStructTag(object.type);
                const normalizedStardustStruct = normalizeStructTag(STARDUST_NFT_OUTPUT_TYPE);
                if (normalizedObjectStruct === normalizedStardustStruct) {
                    const nftDetails = await getNftDetails(object, groupKey, client);
                    flatObjects.push(...nftDetails);
                }
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

    return flatObjects;
}

export function sortStardustResolvedObjectsByExpiration(
    objects: ResolvedObjectTypes[],
    currentEpochStartMs: number,
    currentEpochEndMs: number,
): ResolvedObjectTypes[] {
    const currentTimestampMs = Date.now();

    return objects.sort((a, b) => {
        const aIsNoExpiration = a.unlockConditionTimestamp === MIGRATION_OBJECT_WITHOUT_UC_KEY;
        const bIsNoExpiration = b.unlockConditionTimestamp === MIGRATION_OBJECT_WITHOUT_UC_KEY;

        // No-expiration objects should be last
        if (aIsNoExpiration && bIsNoExpiration) return 0;
        if (aIsNoExpiration) return 1;
        if (bIsNoExpiration) return -1;

        const aTimestampMs = parseInt(a.unlockConditionTimestamp) * MILLISECONDS_PER_SECOND;
        const bTimestampMs = parseInt(b.unlockConditionTimestamp) * MILLISECONDS_PER_SECOND;

        const aIsFromPreviousEpoch = aTimestampMs < currentEpochStartMs;
        const bIsFromPreviousEpoch = bTimestampMs < currentEpochStartMs;

        // Objects from a past epoch should be last (but before no-expiration objects)
        if (aIsFromPreviousEpoch && bIsFromPreviousEpoch) return 0;
        if (aIsFromPreviousEpoch) return 1;
        if (bIsFromPreviousEpoch) return -1;

        const aIsInFutureEpoch = aTimestampMs > currentEpochEndMs;
        const bIsInFutureEpoch = bTimestampMs > currentEpochEndMs;

        const aOutputTimestampMs = aIsInFutureEpoch ? aTimestampMs : currentEpochEndMs;
        const bOutputTimestampMs = bIsInFutureEpoch ? bTimestampMs : currentEpochEndMs;

        // Objects closer to the calculated `outputTimestampMs` should be first
        const aProximity = Math.abs(aOutputTimestampMs - currentTimestampMs);
        const bProximity = Math.abs(bOutputTimestampMs - currentTimestampMs);

        return aProximity - bProximity;
    });
}

async function getNftDetails(
    object: IotaObjectData,
    expirationKey: UnlockConditionTimestamp,
    client: IotaClient,
): Promise<ResolvedNftObject[]> {
    const objectFields = extractOutputFields(object);
    const nftOutputDynamicFields = await client.getDynamicFields({
        parentId: objectFields.id.id,
    });

    const nftDetails: ResolvedNftObject[] = [];
    for (const nft of nftOutputDynamicFields.data) {
        const nftObject = await client.getObject({
            id: nft.objectId,
            options: { showDisplay: true },
        });

        nftDetails.push({
            balance: BigInt(objectFields.balance),
            name: nftObject?.data?.display?.data?.name ?? '',
            image_url: nftObject?.data?.display?.data?.image_url ?? '',
            commonObjectType: CommonMigrationObjectType.Nft,
            unlockConditionTimestamp: expirationKey,
            output: object,
            uniqueId: nftObject?.data?.objectId ?? nft.objectId,
        });
    }

    return nftDetails;
}

async function extractNativeTokensFromObject(
    object: IotaObjectData,
    client: IotaClient,
    expirationKey: UnlockConditionTimestamp,
): Promise<ResolvedNativeToken[]> {
    const fields = extractOutputFields(object);
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
                coinType: nativeTokenFields.name.startsWith('0x')
                    ? nativeTokenFields.name
                    : `0x${nativeTokenFields.name}`,
                unlockConditionTimestamp: expirationKey,
                commonObjectType: CommonMigrationObjectType.NativeToken,
                output: object,
                uniqueId: nativeTokenFields.id.id,
            });
        }
    }

    return result;
}
