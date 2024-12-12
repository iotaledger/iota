// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useGroupedMigrationObjectsByExpirationDate } from '@/hooks';
import {
    MIGRATION_OBJECT_WITHOUT_EXPIRATION_KEY,
    STARDUST_MIGRATION_OBJECTS_FILTER_LIST,
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

interface MigrationObjectsPanelProps {
    selectedObjects: IotaObjectData[];
    onClose: () => void;
    isHidden: boolean;
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
        error: isErrored,
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
                            {isErrored && !isLoading && (
                                <div>
                                    <InfoBox
                                        title="Error"
                                        supportingText="Failed to load migration objects"
                                        style={InfoBoxStyle.Elevated}
                                        type={InfoBoxType.Error}
                                        icon={<Warning />}
                                    />
                                </div>
                            )}
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
}

function ObjectsList({ objects }: ObjectsListProps) {
    return (
        <>
            {objects.map(([expirationUnix, objectList]) => {
                const listKey = getObjectListReactKey(objectList);
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
                    image={
                        <ExternalImage src={migrationObject.image_url} alt={migrationObject.name} />
                    }
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
