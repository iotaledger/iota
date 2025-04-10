// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useGroupedStardustObjects } from '@/hooks';
import {
    STARDUST_MIGRATABLE_OBJECTS_FILTER_LIST,
    STARDUST_TIMELOCKED_OBJECTS_FILTER_LIST,
} from '@/lib/constants';
import { StardustOutputDetailsFilter } from '@/lib/enums';
import {
    Button,
    ButtonType,
    Chip,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    Panel,
    Title,
} from '@iota/apps-ui-kit';
import type { IotaObjectData } from '@iota/iota-sdk/client';
import { Close, Warning } from '@iota/apps-ui-icons';
import clsx from 'clsx';
import { useState } from 'react';
import { MigrationObjectDetailsCard } from './migration-object-details-card';
import { filterMigrationObjects } from '@/lib/utils';
import { MigrationObjectLoading } from './MigrationObjectLoading';
import { VirtualList } from '@iota/core';

const FILTERS = {
    migratable: STARDUST_MIGRATABLE_OBJECTS_FILTER_LIST,
    timelocked: STARDUST_TIMELOCKED_OBJECTS_FILTER_LIST,
};

interface MigrationObjectsPanelProps {
    selectedObjects: IotaObjectData[];
    onClose: () => void;
    groupByTimelockUC: boolean;
}

export function MigrationObjectsPanel({
    selectedObjects,
    onClose,
    groupByTimelockUC,
}: MigrationObjectsPanelProps): React.JSX.Element {
    const [stardustOutputDetailsFilter, setStardustOutputDetailsFilter] =
        useState<StardustOutputDetailsFilter>(StardustOutputDetailsFilter.All);

    const {
        data: resolvedObjects = [],
        isLoading,
        error: isErrored,
    } = useGroupedStardustObjects(selectedObjects, groupByTimelockUC);

    const filteredObjects = filterMigrationObjects(resolvedObjects, stardustOutputDetailsFilter);

    const filters = groupByTimelockUC ? FILTERS.timelocked : FILTERS.migratable;
    const isHidden = selectedObjects.length === 0;

    return (
        <div className={clsx('flex h-full min-h-0 w-full flex-col md:w-2/3', isHidden && 'hidden')}>
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
                            {isLoading && <MigrationObjectLoading />}
                            {isErrored && !isLoading && (
                                <div className="flex h-full max-h-full w-full flex-col items-center">
                                    <InfoBox
                                        title="Error"
                                        supportingText="Failed to load stardust objects"
                                        style={InfoBoxStyle.Elevated}
                                        type={InfoBoxType.Error}
                                        icon={<Warning />}
                                    />
                                </div>
                            )}
                            {!isLoading && !isErrored && (
                                <VirtualList
                                    heightClassName="h-[600px]"
                                    items={filteredObjects}
                                    estimateSize={() => 58}
                                    getItemKey={(migrationObject) => migrationObject.uniqueId}
                                    render={(migrationObject) => (
                                        <MigrationObjectDetailsCard
                                            migrationObject={migrationObject}
                                            isTimelocked={groupByTimelockUC}
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
