// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import {
    Button,
    ButtonType,
    Card,
    CardBody,
    CardImage,
    Chip,
    ImageShape,
    Panel,
    Title,
} from '@iota/apps-ui-kit';
import { Assets, Close, DataStack, IotaLogoMark } from '@iota/ui-icons';
import { ObjectDetailsCategory, ObjectsFilter } from '../enums';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { STARDUST_BASIC_OUTPUT_TYPE, STARDUST_NFT_OUTPUT_TYPE } from '@iota/core';
import { SVGProps } from 'react';

interface MigrationObjectsPanelProps {
    objects: IotaObjectData[];
    setSelectedObjectsCategory: (category: ObjectDetailsCategory | undefined) => void;
    selectedObjectsCategory: ObjectDetailsCategory;
    selectedFilter: ObjectsFilter;
    setSelectedFilter: (filter: ObjectsFilter) => void;
    filters: ObjectsFilter[];
}
export function MigrationObjectsPanel({
    objects,
    setSelectedObjectsCategory,
    selectedObjectsCategory,
    selectedFilter,
    setSelectedFilter,
    filters,
}: MigrationObjectsPanelProps): React.JSX.Element {
    return (
        <div className="flex h-full min-h-0 w-2/3 flex-col [&_>div]:h-full">
            <Panel>
                <Title
                    title="Details"
                    trailingElement={
                        <Button
                            icon={<Close />}
                            type={ButtonType.Ghost}
                            onClick={() => setSelectedObjectsCategory(undefined)}
                        />
                    }
                />
                <div className="flex min-h-0 flex-1 flex-col px-md--rs">
                    <div className="flex flex-row gap-xs py-xs">
                        {filters.map((filter) => (
                            <Chip
                                key={filter}
                                label={filter}
                                onClick={() => setSelectedFilter(filter)}
                                selected={selectedFilter === filter}
                            />
                        ))}
                    </div>
                    <div className="flex min-h-0 flex-col py-sm">
                        <div className="flex-1 overflow-auto">
                            {objects.map((object) => (
                                <ObjectCard key={object.digest} object={object} />
                            ))}
                        </div>
                    </div>
                </div>
            </Panel>
        </div>
    );
}

function getCardSubtitle(type?: string | null): string {
    switch (type) {
        case STARDUST_BASIC_OUTPUT_TYPE:
            return 'IOTA Tokens';
        case STARDUST_NFT_OUTPUT_TYPE:
            return 'Visual Assets';
        default:
            return 'Native Tokens';
    }
}

function getCardIcon(type?: string | null): (props: SVGProps<SVGSVGElement>) => React.ReactNode {
    switch (type) {
        case STARDUST_BASIC_OUTPUT_TYPE:
            return IotaLogoMark;
        case STARDUST_NFT_OUTPUT_TYPE:
            return Assets;
        default:
            return DataStack;
    }
}

function ObjectCard({ object }: { object: IotaObjectData }): React.JSX.Element {
    const subtitle = getCardSubtitle(object.type);
    const Icon = getCardIcon(object.type);

    return (
        <Card>
            <CardImage shape={ImageShape.SquareRounded}>
                <Icon />
            </CardImage>
            <CardBody title={object.digest} subtitle={subtitle} />
        </Card>
    );
}
