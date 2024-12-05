// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import MigratePopup from '@/components/Popup/Popups/MigratePopup';
import { useGetCurrentEpochStartTimestamp, usePopups } from '@/hooks';
import {
    summarizeMigratableObjectValues,
    groupStardustObjectsByMigrationStatus,
} from '@/lib/utils';
import {
    Button,
    ButtonSize,
    ButtonType,
    Card,
    CardBody,
    CardImage,
    ImageShape,
    Panel,
    Title,
} from '@iota/apps-ui-kit';
import { useCurrentAccount, useIotaClient } from '@iota/dapp-kit';
import {
    STARDUST_BASIC_OUTPUT_TYPE,
    STARDUST_NFT_OUTPUT_TYPE,
    useFormatCoin,
    useGetAllOwnedObjects,
} from '@iota/core';
import { useQueryClient } from '@tanstack/react-query';
import { Assets, Clock, IotaLogoMark, Tokens } from '@iota/ui-icons';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useState } from 'react';
import clsx from 'clsx';
import { ObjectDetailsCategory, ObjectsFilter } from './enums';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { MigrationObjectsPanel } from './components/MigrationObjectsPanel';

const FILTER_LIST: ObjectsFilter[] = Object.values(ObjectsFilter);

interface MigrationDisplayCard {
    title: string;
    subtitle: string;
    icon: React.FC;
}

function MigrationDashboardPage(): JSX.Element {
    const account = useCurrentAccount();
    const address = account?.address || '';
    const { openPopup, closePopup } = usePopups();
    const queryClient = useQueryClient();
    const iotaClient = useIotaClient();
    const { data: currentEpochMs } = useGetCurrentEpochStartTimestamp();
    const [selectedObjectsCategory, setSelectedObjectsCategory] = useState<
        ObjectDetailsCategory | undefined
    >();
    const [selectedFilter, setSelectedFilter] = useState<ObjectsFilter>(ObjectsFilter.All);

    const { data: basicOutputObjects } = useGetAllOwnedObjects(address, {
        StructType: STARDUST_BASIC_OUTPUT_TYPE,
    });
    const { data: nftOutputObjects } = useGetAllOwnedObjects(address, {
        StructType: STARDUST_NFT_OUTPUT_TYPE,
    });

    const { migratable: migratableBasicOutputs, unmigratable: unmigratableBasicOutputs } =
        groupStardustObjectsByMigrationStatus(
            basicOutputObjects ?? [],
            Number(currentEpochMs),
            address,
        );

    const { migratable: migratableNftOutputs, unmigratable: unmigratableNftOutputs } =
        groupStardustObjectsByMigrationStatus(
            nftOutputObjects ?? [],
            Number(currentEpochMs),
            address,
        );

    const hasMigratableObjects =
        migratableBasicOutputs.length > 0 || migratableNftOutputs.length > 0;

    function handleOnSuccess(digest: string): void {
        iotaClient
            .waitForTransaction({
                digest,
            })
            .then(() => {
                queryClient.invalidateQueries({
                    queryKey: [
                        'get-all-owned-objects',
                        address,
                        {
                            StructType: STARDUST_BASIC_OUTPUT_TYPE,
                        },
                    ],
                });
                queryClient.invalidateQueries({
                    queryKey: [
                        'get-all-owned-objects',
                        address,
                        {
                            StructType: STARDUST_NFT_OUTPUT_TYPE,
                        },
                    ],
                });
            });
    }
    function openMigratePopup(): void {
        openPopup(
            <MigratePopup
                basicOutputObjects={migratableBasicOutputs}
                nftOutputObjects={migratableNftOutputs}
                closePopup={closePopup}
                onSuccess={handleOnSuccess}
            />,
        );
    }

    const {
        accumulatedIotaAmount: accumulatedTimelockedIotaAmount,
        totalNativeTokens,
        totalVisualAssets,
    } = summarizeMigratableObjectValues({
        migratableBasicOutputs,
        migratableNftOutputs,
        address,
    });

    const [timelockedIotaTokens, symbol] = useFormatCoin(
        accumulatedTimelockedIotaAmount,
        IOTA_TYPE_ARG,
    );

    const MIGRATION_CARDS: MigrationDisplayCard[] = [
        {
            title: `${timelockedIotaTokens} ${symbol}`,
            subtitle: 'IOTA Tokens',
            icon: IotaLogoMark,
        },
        {
            title: `${totalNativeTokens}`,
            subtitle: 'Native Tokens',
            icon: Tokens,
        },
        {
            title: `${totalVisualAssets}`,
            subtitle: 'Visual Assets',
            icon: Assets,
        },
    ];

    const timelockedAssetsAmount = unmigratableBasicOutputs.length + unmigratableNftOutputs.length;
    const TIMELOCKED_ASSETS_CARDS: MigrationDisplayCard[] = [
        {
            title: `${timelockedAssetsAmount}`,
            subtitle: 'Time-locked',
            icon: Clock,
        },
    ];

    const objects = groupSelectedObjectsByFilter();

    function groupSelectedObjectsByFilter(): IotaObjectData[] | undefined {
        if (!selectedObjectsCategory) {
            return;
        }

        if (selectedObjectsCategory === ObjectDetailsCategory.Migration) {
            return groupFilteredMigratableObjects();
        } else {
            return groupFilteredUnmigratableObjects();
        }
    }

    function groupFilteredUnmigratableObjects(): IotaObjectData[] {
        switch (selectedFilter) {
            case ObjectsFilter.NativeTokens:
                return unmigratableBasicOutputs;
            case ObjectsFilter.VisualAssets:
                return unmigratableNftOutputs;
            case ObjectsFilter.All:
            default:
                return [...unmigratableBasicOutputs, ...unmigratableNftOutputs];
        }
    }

    function groupFilteredMigratableObjects(): IotaObjectData[] {
        switch (selectedFilter) {
            case ObjectsFilter.NativeTokens:
                return migratableBasicOutputs;
            case ObjectsFilter.VisualAssets:
                return migratableNftOutputs;
            default:
            case ObjectsFilter.All:
                return [...migratableBasicOutputs, ...migratableNftOutputs];
        }
    }

    return (
        <div className="flex h-full w-full flex-wrap items-center justify-center space-y-4">
            <div
                className={clsx(
                    'flex h-[700px] w-full flex-row items-stretch',
                    !selectedObjectsCategory ? 'justify-center' : 'gap-md--rs',
                )}
            >
                <div className="flex w-1/3 flex-col gap-md--rs">
                    <Panel>
                        <Title
                            title="Migration"
                            trailingElement={
                                <Button
                                    text="Migrate All"
                                    disabled={!hasMigratableObjects}
                                    onClick={openMigratePopup}
                                    size={ButtonSize.Small}
                                />
                            }
                        />
                        <div className="flex flex-col gap-xs p-md--rs">
                            {MIGRATION_CARDS.map((card) => (
                                <Card key={card.subtitle}>
                                    <CardImage shape={ImageShape.SquareRounded}>
                                        <card.icon />
                                    </CardImage>
                                    <CardBody title={card.title} subtitle={card.subtitle} />
                                </Card>
                            ))}
                            <Button
                                text="See All"
                                type={ButtonType.Ghost}
                                fullWidth
                                onClick={() =>
                                    setSelectedObjectsCategory(ObjectDetailsCategory.Migration)
                                }
                            />
                        </div>
                    </Panel>

                    <Panel>
                        <Title title="Time-locked Assets" />
                        <div className="flex flex-col gap-xs p-md--rs">
                            {TIMELOCKED_ASSETS_CARDS.map((card) => (
                                <Card key={card.subtitle}>
                                    <CardImage shape={ImageShape.SquareRounded}>
                                        <card.icon />
                                    </CardImage>
                                    <CardBody title={card.title} subtitle={card.subtitle} />
                                </Card>
                            ))}
                            <Button
                                text="See All"
                                type={ButtonType.Ghost}
                                fullWidth
                                onClick={() =>
                                    setSelectedObjectsCategory(ObjectDetailsCategory.TimeLocked)
                                }
                            />
                        </div>
                    </Panel>
                </div>

                {selectedObjectsCategory && objects && (
                    <MigrationObjectsPanel
                        objects={objects}
                        selectedFilter={selectedFilter}
                        setSelectedFilter={setSelectedFilter}
                        setSelectedObjectsCategory={setSelectedObjectsCategory}
                        filters={FILTER_LIST}
                        selectedObjectsCategory={selectedObjectsCategory}
                    />
                )}
            </div>
        </div>
    );
}

export default MigrationDashboardPage;
