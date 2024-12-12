// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ExpirationObjectListEntries, ResolvedObjectsList } from '@/lib/types';
import { getObjectListReactKey, isNftObjectList, isObjectTypeBasic } from '../helpers';
import { ResolvedObjectCard } from './ResolvedObjectCard';

interface ObjectsListProps {
    objects: ExpirationObjectListEntries;
    isTimelockedObjects: boolean;
}

export function ResolvedMigrationObjectList({ objects, isTimelockedObjects }: ObjectsListProps) {
    return (
        <>
            {objects.map(([expirationUnix, objectList]) => {
                const listKey = getObjectListReactKey(objectList);
                return (
                    <ObjectListRenderer
                        objectList={objectList}
                        key={`${expirationUnix} ${listKey}`}
                        isTimelockedObjects={isTimelockedObjects}
                    />
                );
            })}
        </>
    );
}

function ObjectListRenderer({
    objectList,
    isTimelockedObjects,
}: {
    objectList: ResolvedObjectsList;
    isTimelockedObjects: boolean;
}) {
    if (isNftObjectList(objectList)) {
        return objectList.map((nft) => (
            <ResolvedObjectCard
                migrationObject={nft}
                key={nft.uniqueId}
                isTimelockedObjects={isTimelockedObjects}
            />
        ));
    } else if (isObjectTypeBasic(objectList)) {
        return (
            <ResolvedObjectCard
                migrationObject={objectList}
                isTimelockedObjects={isTimelockedObjects}
            />
        );
    } else {
        return Object.values(objectList).map((nativeToken) => (
            <ResolvedObjectCard
                migrationObject={nativeToken}
                key={nativeToken.uniqueId}
                isTimelockedObjects={isTimelockedObjects}
            />
        ));
    }
}
