// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import {
    useGetCurrentEpochStartTimestamp,
    useGroupedMigrationObjectsByExpirationDate,
} from '@/hooks';
import {
    MIGRATION_OBJECT_WITHOUT_EXPIRATION_KEY,
    STARDUST_MIGRATABLE_OBJECTS_FILTER_LIST,
    STARDUST_UNMIGRATABLE_OBJECTS_FILTER_LIST,
} from '@/lib/constants';
import { CommonMigrationObjectType, StardustObjectTypeFilter } from '@/lib/enums';
import {
    ExpirationObjectListEntries,
    ResolvedObjectsGrouped,
    ResolvedObjectsList,
    ResolvedObjectTypes,
} from '@/lib/types';
import {
    Button,
    ButtonType,
    Card,
    CardBody,
    CardImage,
    Chip,
    ImageShape,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    LabelText,
    LabelTextSize,
    Panel,
    Title,
} from '@iota/apps-ui-kit';
import { TimeUnit, useFormatCoin, useTimeAgo } from '@iota/core';
import type { IotaObjectData } from '@iota/iota-sdk/client';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Close, DataStack, IotaLogoMark, Warning } from '@iota/ui-icons';
import clsx from 'clsx';
import { useMemo, useState } from 'react';
import { ExternalImage, SkeletonLoader } from '..';
import {
    getAllResolvedObjects,
    getObjectListReactKey,
    getObjectsWithExpiration,
    isNftObjectList,
    isObjectTypeBasic,
} from './helpers';

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
                                <ObjectsList
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

interface ObjectsListProps {
    objects: ExpirationObjectListEntries;
    isTimelockedObjects: boolean;
}

function ObjectsList({ objects, isTimelockedObjects }: ObjectsListProps) {
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

function ResolvedObjectCard({
    migrationObject,
    isTimelockedObjects,
}: {
    migrationObject: ResolvedObjectTypes;
    isTimelockedObjects: boolean;
}) {
    const coinType = 'coinType' in migrationObject ? migrationObject.coinType : IOTA_TYPE_ARG;
    const [balance, token] = useFormatCoin(migrationObject.balance, coinType);

    switch (migrationObject.commonObjectType) {
        case CommonMigrationObjectType.Basic:
            return (
                <MigrationObjectCard
                    title={`${balance} ${token}`}
                    subtitle="IOTA Tokens"
                    expirationKey={migrationObject.expirationKey}
                    image={<IotaLogoMark />}
                    isTimelockedObjects={isTimelockedObjects}
                />
            );
        case CommonMigrationObjectType.Nft:
            return (
                <MigrationObjectCard
                    title={migrationObject.name}
                    subtitle="Visual Assets"
                    expirationKey={migrationObject.expirationKey}
                    image={
                        <ExternalImage src={migrationObject.image_url} alt={migrationObject.name} />
                    }
                    isTimelockedObjects={isTimelockedObjects}
                />
            );
        case CommonMigrationObjectType.NativeToken:
            return (
                <MigrationObjectCard
                    isTimelockedObjects={isTimelockedObjects}
                    title={`${balance} ${token}`}
                    subtitle="Native Tokens"
                    expirationKey={migrationObject.expirationKey}
                    image={<DataStack />}
                />
            );
        default:
            return null;
    }
}

interface MigrationObjectCardProps {
    title: string;
    subtitle: string;
    expirationKey: string;
    image?: React.ReactNode;
    isTimelockedObjects: boolean;
}

function MigrationObjectCard({
    title,
    subtitle,
    image,
    expirationKey,
    isTimelockedObjects,
}: MigrationObjectCardProps) {
    const hasExpiration = expirationKey !== MIGRATION_OBJECT_WITHOUT_EXPIRATION_KEY;
    return (
        <Card>
            <CardImage shape={ImageShape.SquareRounded}>{image}</CardImage>
            <CardBody title={title} subtitle={subtitle} />
            {hasExpiration && (
                <ExpirationDate
                    groupKey={expirationKey}
                    isTimelockedObjects={isTimelockedObjects}
                />
            )}
        </Card>
    );
}

function ExpirationDate({
    groupKey,
    isTimelockedObjects,
}: {
    groupKey: string;
    isTimelockedObjects: boolean;
}) {
    const { data: epochTimestamp } = useGetCurrentEpochStartTimestamp();
    const timeAgo = useTimeAgo({
        timeFrom: Number(groupKey) * 1000,
        shortedTimeLabel: true,
        shouldEnd: true,
        maxTimeUnit: TimeUnit.ONE_DAY,
    });

    const showTimestamp = Number(groupKey) < Number(epochTimestamp);

    return (
        <div className="ml-auto h-full whitespace-nowrap">
            {showTimestamp && (
                <LabelText
                    size={LabelTextSize.Small}
                    text={timeAgo}
                    label={isTimelockedObjects ? 'Unlocks in' : 'Expires in'}
                />
            )}
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
