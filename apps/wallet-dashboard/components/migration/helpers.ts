// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { MIGRATION_OBJECT_WITHOUT_EXPIRATION_KEY } from '@/lib/constants';
import { CommonMigrationObjectType } from '@/lib/enums';
import {
    ExpirationObjectListEntries,
    NftObjectsResolvedList,
    ResolvedBasicObject,
    ResolvedObjectsGrouped,
    ResolvedObjectsList,
} from '@/lib/types';

export function getObjectListReactKey(objectList: ResolvedObjectsList): string {
    if (Array.isArray(objectList)) {
        return CommonMigrationObjectType.Nft;
    } else if (isObjectTypeBasic(objectList)) {
        return CommonMigrationObjectType.Basic;
    } else {
        return CommonMigrationObjectType.NativeToken;
    }
}

export function getAllResolvedObjects(
    resolvedObjects: ResolvedObjectsGrouped,
): ExpirationObjectListEntries {
    return [
        ...Object.entries(resolvedObjects.basicObjects),
        ...Object.entries(resolvedObjects.nftObjects),
        ...Object.entries(resolvedObjects.nativeTokens),
    ];
}

export function getObjectsWithExpiration(
    resolvedObjects: ResolvedObjectsGrouped,
): ExpirationObjectListEntries {
    const allEntries = getAllResolvedObjects(resolvedObjects);
    return allEntries.filter(
        ([expiration]) => expiration !== MIGRATION_OBJECT_WITHOUT_EXPIRATION_KEY,
    );
}

export function isObjectTypeBasic(object: ResolvedObjectsList): object is ResolvedBasicObject {
    return (
        !Array.isArray(object) &&
        'commonObjectType' in object &&
        object.commonObjectType === CommonMigrationObjectType.Basic
    );
}

export function isNftObjectList(
    objectList: ResolvedObjectsList,
): objectList is NftObjectsResolvedList {
    return Array.isArray(objectList);
}
