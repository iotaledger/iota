// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useGroupedMigrationObjectsByExpirationDate } from '@/hooks';
import {
    STARDUST_MIGRATABLE_OBJECTS_FILTER_LIST,
    STARDUST_UNMIGRATABLE_OBJECTS_FILTER_LIST,
} from '@/lib/constants';
import { StardustObjectTypeFilter } from '@/lib/enums';
import type { ExpirationObjectListEntries, ResolvedObjectsGrouped } from '@/lib/types';
import {
    Button,
    ButtonType,
    Card,
    CardImage,
    Chip,
    ImageShape,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    Panel,
    Title,
} from '@iota/apps-ui-kit';
import type { IotaObjectData } from '@iota/iota-sdk/client';
import { Close, Warning } from '@iota/ui-icons';
import clsx from 'clsx';
import { useMemo, useState } from 'react';
import { SkeletonLoader } from '..';
import { getAllResolvedObjects, getObjectsWithExpiration } from './helpers';
import { ResolvedMigrationObjectList } from './resolved-migration-objects';

const FILTERS = {
    migratable: STARDUST_MIGRATABLE_OBJECTS_FILTER_LIST,
    unmigratable: STARDUST_UNMIGRATABLE_OBJECTS_FILTER_LIST,
};

interface MigrationObjectsPanelProps {
    selectedObjects: IotaObjectData[];
    onClose: () => void;
    isHidden: boolean;
    isTimelockedObjects: boolean;
}

export function MigrationObjectsPanel({
    selectedObjects,
    onClose,
    isHidden,
    isTimelockedObjects,
}: MigrationObjectsPanelProps): React.JSX.Element {
    const [selectedStardustObjectType, setSelectedStardustObjectType] =
        useState<StardustObjectTypeFilter>(StardustObjectTypeFilter.All);

    const {
        data: resolvedObjects,
        isLoading,
        error: isErrored,
    } = useGroupedMigrationObjectsByExpirationDate(selectedObjects, isTimelockedObjects);

    const filteredObjects = useGetFilteredObjects(resolvedObjects, selectedStardustObjectType);
    const filters = isTimelockedObjects ? FILTERS.unmigratable : FILTERS.migratable;

    return (
        <div className={clsx('flex h-full min-h-0 w-2/3 flex-col', isHidden && 'hidden')}>
            <Panel>
                <Title
                    title="Details"
                    trailingElement={
                        <Button icon={<Close />} type={ButtonType.Ghost} onClick={onClose} />
                    }
                />
                <div className="flex min-h-0 flex-1 flex-col px-md--rs">
                    <div className="flex flex-row gap-xs py-xs">
                        {filters.map((filter) => (
                            <Chip
                                key={filter}
                                label={filter}
                                onClick={() => setSelectedStardustObjectType(filter)}
                                selected={selectedStardustObjectType === filter}
                            />
                        ))}
                    </div>
                    <div className="flex h-[600px] min-h-0 flex-col py-sm">
                        <div className="h-full flex-1 overflow-auto">
                            {isLoading && <LoadingPanel />}
                            {isErrored && !isLoading && (
                                <div className="flex h-full max-h-full w-full flex-col items-center">
                                    <InfoBox
                                        title="Error"
                                        supportingText="Failed to load migration objects"
                                        style={InfoBoxStyle.Elevated}
                                        type={InfoBoxType.Error}
                                        icon={<Warning />}
                                    />
                                </div>
                            )}
                            {!isLoading && !isErrored && (
                                <ResolvedMigrationObjectList
                                    objects={filteredObjects}
                                    isTimelockedObjects={isTimelockedObjects}
                                />
                            )}
                        </div>
                    </div>
                </div>
            </Panel>
        </div>
    );
}

function LoadingPanel() {
    return (
        <div className="flex h-full max-h-full w-full flex-col overflow-hidden">
            {new Array(10).fill(0).map((_, index) => (
                <Card key={index}>
                    <CardImage shape={ImageShape.SquareRounded}>
                        <div className="h-10 w-10 animate-pulse bg-neutral-90 dark:bg-neutral-12" />
                        <SkeletonLoader widthClass="w-10" heightClass="h-10" isRounded={false} />
                    </CardImage>
                    <div className="flex flex-col gap-y-xs">
                        <SkeletonLoader widthClass="w-40" heightClass="h-3.5" />
                        <SkeletonLoader widthClass="w-32" heightClass="h-3" hasSecondaryColors />
                    </div>
                    <div className="ml-auto flex flex-col gap-y-xs">
                        <SkeletonLoader widthClass="w-20" heightClass="h-3.5" />
                        <SkeletonLoader widthClass="w-16" heightClass="h-3" hasSecondaryColors />
                    </div>
                </Card>
            ))}
        </div>
    );
}

function useGetFilteredObjects(
    objects: ResolvedObjectsGrouped | undefined,
    filter: StardustObjectTypeFilter,
): ExpirationObjectListEntries {
    return useMemo(() => {
        const objectsMap = objects ?? {
            nftObjects: {},
            basicObjects: {},
            nativeTokens: {},
        };

        switch (filter) {
            case StardustObjectTypeFilter.All:
                return getAllResolvedObjects(objectsMap);
            case StardustObjectTypeFilter.IOTA:
                return Object.entries(objectsMap.basicObjects);
            case StardustObjectTypeFilter.VisualAssets:
                return Object.entries(objectsMap.nftObjects);
            case StardustObjectTypeFilter.NativeTokens:
                return Object.entries(objectsMap.nativeTokens);
            case StardustObjectTypeFilter.WithExpiration:
                return getObjectsWithExpiration(objectsMap);
        }
    }, [objects, filter]);
}
