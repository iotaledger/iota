// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useMemo, useState } from 'react';
import {
    Button,
    ButtonType,
    Card,
    CardBody,
    CardImage,
    Chip,
    ImageShape,
    LabelText,
    LabelTextSize,
    LoadingIndicator,
    Panel,
    Title,
} from '@iota/apps-ui-kit';
import { Close, DataStack, IotaLogoMark } from '@iota/ui-icons';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useGroupedMigrationObjectsByExpirationDate } from '@/hooks';
import { TimeUnit, useFormatCoin, useTimeAgo } from '@iota/core';
import {
    MIGRATION_OBJECT_WITHOUT_EXPIRATION_KEY,
    STARDUST_MIGRATION_OBJECTS_FILTER_LIST,
} from '@/lib/constants';
import type { IotaObjectData } from '@iota/iota-sdk/client';
import { CommonMigrationObjectType, StardustObjectTypeFilter } from '@/lib/enums';
import {
    ExpirationObjectListEntries,
    NftObjectsResolvedList,
    ResolvedBasicObject,
    ResolvedObjectsList,
    ResolvedObjectTypes,
    ResolvedObjectsGrouped,
} from '@/lib/types';
import clsx from 'clsx';

interface MigrationObjectsPanelProps {
    selectedObjects: IotaObjectData[];
    onClose: () => void;
    isHidden: boolean;
}

function useGetFilteredObjects(
    objects: ResolvedObjectsGrouped,
    filter: StardustObjectTypeFilter,
): ExpirationObjectListEntries {
    return useMemo(() => {
        switch (filter) {
            case StardustObjectTypeFilter.All:
                return getAllResolvedObjects(objects);
            case StardustObjectTypeFilter.IOTA:
                return Object.entries(objects.basicObjects);
            case StardustObjectTypeFilter.VisualAssets:
                return Object.entries(objects.nftObjects);
            case StardustObjectTypeFilter.NativeTokens:
                return Object.entries(objects.nativeTokens);
            case StardustObjectTypeFilter.WithExpiration:
                return getObjectsWithExpiration(objects);
        }
    }, [objects, filter]);
}

export function MigrationObjectsPanel({
    selectedObjects,
    onClose,
    isHidden,
}: MigrationObjectsPanelProps): React.JSX.Element {
    const [selectedStardustObjectType, setSelectedStardustObjectType] =
        useState<StardustObjectTypeFilter>(StardustObjectTypeFilter.All);

    const {
        data: resolvedObjects,
        isLoading,
        isErrored,
    } = useGroupedMigrationObjectsByExpirationDate(selectedObjects);

    const filteredObjects = useGetFilteredObjects(resolvedObjects, selectedStardustObjectType);

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
                        {STARDUST_MIGRATION_OBJECTS_FILTER_LIST.map((filter) => (
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
                            {isErrored && !isLoading && <div>Error</div>}
                            {!isLoading && !isErrored && <ObjectsList objects={filteredObjects} />}
                        </div>
                    </div>
                </div>
            </Panel>
        </div>
    );
}

function LoadingPanel() {
    return (
        <div className="flex h-full w-full flex-col items-center justify-center">
            <LoadingIndicator />
        </div>
    );
}

function getObjectListKey(objectList: ResolvedObjectsList): string {
    if (Array.isArray(objectList)) {
        return CommonMigrationObjectType.Nft;
    } else if (isObjectTypeBasic(objectList)) {
        return CommonMigrationObjectType.Basic;
    } else {
        return CommonMigrationObjectType.NativeToken;
    }
}

interface ObjectsListProps {
    objects: ExpirationObjectListEntries;
}

function ObjectsList({ objects }: ObjectsListProps) {
    return (
        <>
            {objects.map(([expirationUnix, objectList]) => {
                const listKey = getObjectListKey(objectList);
                return (
                    <ObjectListRenderer
                        objectList={objectList}
                        key={`${expirationUnix} ${listKey}`}
                    />
                );
            })}
        </>
    );
}

function ObjectListRenderer({ objectList }: { objectList: ResolvedObjectsList }) {
    if (isNftObjectList(objectList)) {
        return objectList.map((nft) => (
            <ResolvedObjectCard migrationObject={nft} key={nft.uniqueId} />
        ));
    } else if (isObjectTypeBasic(objectList)) {
        return <ResolvedObjectCard migrationObject={objectList} />;
    } else {
        return Object.values(objectList).map((nativeToken) => (
            <ResolvedObjectCard migrationObject={nativeToken} key={nativeToken.uniqueId} />
        ));
    }
}

function ResolvedObjectCard({ migrationObject }: { migrationObject: ResolvedObjectTypes }) {
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
                />
            );
        case CommonMigrationObjectType.Nft:
            return (
                <MigrationObjectCard
                    title={migrationObject.name}
                    subtitle="Visual Assets"
                    expirationKey={migrationObject.expirationKey}
                    image={<img src={migrationObject.image_url} alt={migrationObject.name} />}
                />
            );
        case CommonMigrationObjectType.NativeToken:
            return (
                <MigrationObjectCard
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
}

function MigrationObjectCard({ title, subtitle, image, expirationKey }: MigrationObjectCardProps) {
    const hasExpiration = expirationKey !== MIGRATION_OBJECT_WITHOUT_EXPIRATION_KEY;
    return (
        <Card>
            <CardImage shape={ImageShape.SquareRounded}>{image}</CardImage>
            <CardBody title={title} subtitle={subtitle} />
            {hasExpiration && <ExpirationDate expirationKey={expirationKey} />}
        </Card>
    );
}

function ExpirationDate({ expirationKey }: { expirationKey: string }) {
    const timeAgo = useTimeAgo({
        timeFrom: Number(expirationKey),
        shortedTimeLabel: true,
        shouldEnd: true,
        maxTimeUnit: TimeUnit.ONE_DAY,
    });

    return (
        <div className="ml-auto h-full whitespace-nowrap">
            <LabelText size={LabelTextSize.Small} text={timeAgo} label={'Expires in'} />
        </div>
    );
}

function getAllResolvedObjects(
    resolvedObjects: ResolvedObjectsGrouped,
): ExpirationObjectListEntries {
    return [
        ...Object.entries(resolvedObjects.basicObjects),
        ...Object.entries(resolvedObjects.nftObjects),
        ...Object.entries(resolvedObjects.nativeTokens),
    ];
}

function getObjectsWithExpiration(
    resolvedObjects: ResolvedObjectsGrouped,
): ExpirationObjectListEntries {
    const allEntries = getAllResolvedObjects(resolvedObjects);
    return allEntries.filter(
        ([expiration]) => expiration !== MIGRATION_OBJECT_WITHOUT_EXPIRATION_KEY,
    );
}

function isObjectTypeBasic(object: ResolvedObjectsList): object is ResolvedBasicObject {
    return (
        !Array.isArray(object) &&
        'commonObjectType' in object &&
        object.commonObjectType === CommonMigrationObjectType.Basic
    );
}

function isNftObjectList(objectList: ResolvedObjectsList): objectList is NftObjectsResolvedList {
    return Array.isArray(objectList);
}
