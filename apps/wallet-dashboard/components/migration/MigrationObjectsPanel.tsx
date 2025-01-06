// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useGroupedMigrationObjectsByExpirationDate } from '@/hooks';
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
import { Close, Warning } from '@iota/ui-icons';
import clsx from 'clsx';
import { useState } from 'react';
import { MigrationObjectDetailsCard } from './migration-object-details-card';
import { VirtualList } from '../VirtualList';
import { filterMigrationObjects } from '@/lib/utils';
import { MigrationObjectLoading } from './MigrationObjectLoading';

const FILTERS = {
    migratable: STARDUST_MIGRATABLE_OBJECTS_FILTER_LIST,
    timelocked: STARDUST_TIMELOCKED_OBJECTS_FILTER_LIST,
};

interface MigrationObjectsPanelProps {
    selectedObjects: IotaObjectData[];
    onClose: () => void;
    isTimelocked: boolean;
}

export function MigrationObjectsPanel({
    selectedObjects,
    onClose,
    isTimelocked,
}: MigrationObjectsPanelProps): React.JSX.Element {
    const [stardustOutputDetailsFilter, setStardustOutputDetailsFilter] =
        useState<StardustOutputDetailsFilter>(StardustOutputDetailsFilter.All);

    const {
        data: resolvedObjects = [],
        isLoading,
        error: isErrored,
    } = useGroupedMigrationObjectsByExpirationDate(selectedObjects, isTimelocked);

    const filteredObjects = filterMigrationObjects(resolvedObjects, stardustOutputDetailsFilter);

    const filters = isTimelocked ? FILTERS.timelocked : FILTERS.migratable;
    const isHidden = selectedObjects.length === 0;

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
