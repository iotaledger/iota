// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useFilterMigrationObjects, useGroupedMigrationObjectsByExpirationDate } from '@/hooks';
import {
    STARDUST_MIGRATABLE_OBJECTS_FILTER_LIST,
    STARDUST_UNMIGRATABLE_OBJECTS_FILTER_LIST,
} from '@/lib/constants';
import { StardustOutputDetailsFilter } from '@/lib/enums';
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
    Skeleton,
    Title,
} from '@iota/apps-ui-kit';
import type { IotaObjectData } from '@iota/iota-sdk/client';
import { Close, Warning } from '@iota/ui-icons';
import clsx from 'clsx';
import { useState } from 'react';
import { MigrationObjectDetailsCard } from './migration-object-details-card';
import VirtualList from '../VirtualList';

const FILTERS = {
    migratable: STARDUST_MIGRATABLE_OBJECTS_FILTER_LIST,
    unmigratable: STARDUST_UNMIGRATABLE_OBJECTS_FILTER_LIST,
};

interface MigrationObjectsPanelProps {
    selectedObjects: IotaObjectData[];
    onClose: () => void;
    isHidden: boolean;
    isTimelocked: boolean;
}

export function MigrationObjectsPanel({
    selectedObjects,
    onClose,
    isHidden,
    isTimelocked,
}: MigrationObjectsPanelProps): React.JSX.Element {
    const [stardustOutputDetailsFilter, setStardustOutputDetailsFilter] =
        useState<StardustOutputDetailsFilter>(StardustOutputDetailsFilter.All);

    const {
        data: resolvedObjects = [],
        isLoading,
        error: isErrored,
    } = useGroupedMigrationObjectsByExpirationDate(selectedObjects, isTimelocked);

    const filteredObjects = useFilterMigrationObjects(resolvedObjects, stardustOutputDetailsFilter);

    const filters = isTimelocked ? FILTERS.unmigratable : FILTERS.migratable;

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
                                onClick={() => setStardustOutputDetailsFilter(filter)}
                                selected={stardustOutputDetailsFilter === filter}
                            />
                        ))}
                    </div>
                    <div className="flex min-h-0 flex-col py-sm">
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
                                <VirtualList
                                    heightClassName="h-[600px]"
                                    overflowClassName="overflow-y-auto"
                                    items={filteredObjects}
                                    estimateSize={() => 58}
                                    render={(migrationObject) => (
                                        <MigrationObjectDetailsCard
                                            migrationObject={migrationObject}
                                            isTimelocked={isTimelocked}
                                        />
                                    )}
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
                        <Skeleton widthClass="w-10" heightClass="h-10" isRounded={false} />
                    </CardImage>
                    <div className="flex flex-col gap-y-xs">
                        <Skeleton widthClass="w-40" heightClass="h-3.5" />
                        <Skeleton widthClass="w-32" heightClass="h-3" hasSecondaryColors />
                    </div>
                    <div className="ml-auto flex flex-col gap-y-xs">
                        <Skeleton widthClass="w-20" heightClass="h-3.5" />
                        <Skeleton widthClass="w-16" heightClass="h-3" hasSecondaryColors />
                    </div>
                </Card>
            ))}
        </div>
    );
}
