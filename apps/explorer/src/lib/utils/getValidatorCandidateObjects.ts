// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type {
    DynamicFieldInfo,
    IotaClient,
    IotaObjectDataOptions,
    IotaObjectResponse,
} from '@iota/iota-sdk/client';
import { normalizeIotaAddress } from '@iota/iota-sdk/utils';

const MULTI_GET_OBJECTS_LIMIT = 50;

async function fetchAllDynamicFields(
    client: IotaClient,
    parentId: string,
): Promise<DynamicFieldInfo[]> {
    const allEntries: DynamicFieldInfo[] = [];
    let cursor: string | null | undefined = null;
    let hasNextPage = true;

    while (hasNextPage) {
        const page = await client.getDynamicFields({
            parentId,
            ...(cursor ? { cursor } : {}),
        });
        allEntries.push(...page.data);
        cursor = page.nextCursor;
        hasNextPage = page.hasNextPage;
    }

    return allEntries;
}

/**
 * Batch wrapper around `client.multiGetObjects` that respects the
 * RPC limit of 50 objects per request.
 */
async function batchMultiGetObjects(
    client: IotaClient,
    ids: string[],
    options?: IotaObjectDataOptions,
): Promise<IotaObjectResponse[]> {
    const results: IotaObjectResponse[] = [];

    for (let i = 0; i < ids.length; i += MULTI_GET_OBJECTS_LIMIT) {
        const batch = ids.slice(i, i + MULTI_GET_OBJECTS_LIMIT);
        const batchResults = await client.multiGetObjects({ ids: batch, options });
        results.push(...batchResults);
    }

    return results;
}

export async function getValidatorCandidateObjects(
    client: IotaClient,
    validatorCandidatesId: string,
): Promise<IotaObjectResponse[]> {
    const candidateEntries = await fetchAllDynamicFields(
        client,
        normalizeIotaAddress(validatorCandidatesId),
    );

    if (candidateEntries.length === 0) {
        return [];
    }

    // Fetch wrapper objects to get the inner Versioned IDs
    const wrapperObjects = await batchMultiGetObjects(
        client,
        candidateEntries.map((entry) => normalizeIotaAddress(entry.objectId)),
        { showContent: true },
    );

    // Extract the inner Versioned object IDs from each wrapper
    const innerIds = wrapperObjects
        .map((obj) => {
            const content = obj.data?.content;
            if (content?.dataType !== 'moveObject') return null;
            const fields = content.fields as Record<string, unknown>;
            const value = fields?.value as Record<string, unknown> | undefined;
            const inner = (value?.fields as Record<string, unknown>)?.inner as
                | Record<string, unknown>
                | undefined;
            const innerFields = inner?.fields as Record<string, unknown> | undefined;
            const id = innerFields?.id as Record<string, string> | undefined;
            return id?.id ?? null;
        })
        .filter((id): id is string => id !== null);

    if (innerIds.length === 0) {
        return [];
    }

    // Fetch the ValidatorV1 dynamic field objects from each Versioned object
    // getDynamicFieldObject only returns the object reference (id, version, digest),
    // so we first resolve the object IDs then fetch with showContent.
    const dynamicFieldRefs = await Promise.all(
        innerIds.map((innerId) =>
            client.getDynamicFieldObject({
                parentObjectId: normalizeIotaAddress(innerId),
                name: { type: 'u64', value: '1' },
            }),
        ),
    );

    const validatorV1ObjectIds = dynamicFieldRefs
        .map((ref) => ref.data?.objectId)
        .filter((id): id is string => !!id);

    if (validatorV1ObjectIds.length === 0) {
        return [];
    }

    const validatorV1Objects = await batchMultiGetObjects(client, validatorV1ObjectIds, {
        showContent: true,
    });

    return validatorV1Objects;
}
