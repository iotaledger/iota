// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { MIGRATION_OBJECT_WITHOUT_UC_KEY } from '@/lib/constants';
import { CommonMigrationObjectType, StardustOutputDetailsFilter } from '@/lib/enums';
import { ResolvedObjectTypes } from '@/lib/types';
import { useMemo } from 'react';

export function useFilterMigrationObjects(
    objects: ResolvedObjectTypes[],
    filter: StardustOutputDetailsFilter,
) {
    return useMemo(() => {
        switch (filter) {
            case StardustOutputDetailsFilter.All:
                return objects;
            case StardustOutputDetailsFilter.IOTA:
                return filterObjectByCommonOutputType(objects, CommonMigrationObjectType.Basic);
            case StardustOutputDetailsFilter.VisualAssets:
                return filterObjectByCommonOutputType(objects, CommonMigrationObjectType.Nft);
            case StardustOutputDetailsFilter.NativeTokens:
                return filterObjectByCommonOutputType(
                    objects,
                    CommonMigrationObjectType.NativeToken,
                );
            case StardustOutputDetailsFilter.WithExpiration:
                return filterObjectsByExpiration(objects);
        }
    }, [objects, filter]);
}

function filterObjectByCommonOutputType(
    objects: ResolvedObjectTypes[],
    type: CommonMigrationObjectType,
) {
    return objects.filter((object) => object.commonObjectType === type);
}

function filterObjectsByExpiration(objects: ResolvedObjectTypes[]): ResolvedObjectTypes[] {
    return objects.filter(
        (object) => object.unlockConditionTimestamp !== MIGRATION_OBJECT_WITHOUT_UC_KEY,
    );
}
