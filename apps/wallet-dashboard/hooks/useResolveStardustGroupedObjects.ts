// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { extractMigrationOutputFields } from '@/lib/utils';
import { STARDUST_BASIC_OUTPUT_TYPE, STARDUST_NFT_OUTPUT_TYPE } from '@iota/core';
import { IotaObjectData } from '@iota/iota-sdk/client';

enum StardustObjectType {
    BasicOutput,
    NftOutput,
    NativeToken,
}

interface ResolvedStartustObject {
    type: StardustObjectType;
    title: string;
    expirationMs: number;
    object: IotaObjectData;
}
export function useResolveStardustGroupedObjects(
    objects: IotaObjectData[],
): ResolvedStartustObject[] {
    const resolvedObjects: ResolvedStartustObject[] = [];

    for (const output of objects) {
        const fields = extractMigrationOutputFields(output);
        const expirationMs = fields.expiration_uc?.fields.unix_time || 0;

        if (output.type === STARDUST_BASIC_OUTPUT_TYPE) {
            resolvedObjects.push({
                type: StardustObjectType.BasicOutput,
                title: 'Basic Output',
                expirationMs,
                object: output,
            });
        } else if (output.type === STARDUST_NFT_OUTPUT_TYPE) {
            resolvedObjects.push({
                type: StardustObjectType.NftOutput,
                title: 'NFT Output',
                expirationMs,
                object: output,
            });
        } else {
            resolvedObjects.push({
                type: StardustObjectType.NativeToken,
                title: 'IOTA Tokens',
                expirationMs,
                object: output,
            });
        }
    }

    return resolvedObjects;
}
