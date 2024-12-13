// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useState, useMemo, useCallback } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import clsx from 'clsx';
import MigratePopup from '@/components/Popup/Popups/MigratePopup';
import { useGetStardustMigratableObjects, usePopups } from '@/hooks';
import { summarizeMigratableObjectValues, summarizeUnmigratableObjectValues } from '@/lib/utils';
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
import { Assets, Clock, IotaLogoMark, Tokens } from '@iota/ui-icons';
import { useCurrentAccount, useIotaClient } from '@iota/dapp-kit';
import { STARDUST_BASIC_OUTPUT_TYPE, STARDUST_NFT_OUTPUT_TYPE, useFormatCoin } from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { StardustOutputMigrationStatus } from '@/lib/enums';
import { MigrationObjectsPanel } from '@/components';

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

    const [selectedStardustObjectsCategory, setSelectedStardustObjectsCategory] = useState<
        StardustOutputMigrationStatus | undefined
    >(undefined);

    const { data: stardustMigrationObjects, isLoading } = useGetStardustMigratableObjects(address);
    const { migratableBasicOutputs, migratableNftOutputs } = stardustMigrationObjects || {};

    const {
        totalIotaAmount,
        totalNativeTokens: migratableNativeTokens,
        totalVisualAssets: migratableVisualAssets,
        totalUnmigratableObjects,
    } = useMemo(() => {
        const {
            migratableBasicOutputs,
            migratableNftOutputs,
            unmigratableBasicOutputs,
            unmigratableNftOutputs,
        } = stardustMigrationObjects || {};

        const migrationValues = summarizeMigratableObjectValues({
            basicOutputs: migratableBasicOutputs,
            nftOutputs: migratableNftOutputs,
            address,
        });
        const unmigratableValues = summarizeUnmigratableObjectValues({
            basicOutputs: unmigratableBasicOutputs,
            nftOutputs: unmigratableNftOutputs,
        });
        return { ...migrationValues, ...unmigratableValues };
    }, [stardustMigrationObjects, address]);

    const hasMigratableObjects =
        (migratableBasicOutputs?.length || 0) > 0 && (migratableNftOutputs?.length || 0) > 0;

    const [timelockedIotaTokens, symbol] = useFormatCoin(totalIotaAmount, IOTA_TYPE_ARG);

    const handleOnSuccess = useCallback(
        (digest: string) => {
            iotaClient.waitForTransaction({ digest }).then(() => {
                queryClient.invalidateQueries({
                    queryKey: [
                        'get-all-owned-objects',
                        address,
                        { StructType: STARDUST_BASIC_OUTPUT_TYPE },
                    ],
                });
                queryClient.invalidateQueries({
                    queryKey: [
                        'get-all-owned-objects',
                        address,
                        { StructType: STARDUST_NFT_OUTPUT_TYPE },
                    ],
                });
            });
        },
        [iotaClient, queryClient, address],
    );

    const MIGRATION_CARDS: MigrationDisplayCard[] = [
        {
            title: `${timelockedIotaTokens} ${symbol}`,
            subtitle: 'IOTA Tokens',
            icon: IotaLogoMark,
        },
        {
            title: `${migratableNativeTokens}`,
            subtitle: 'Native Tokens',
            icon: Tokens,
        },
        {
            title: `${migratableVisualAssets}`,
            subtitle: 'Visual Assets',
            icon: Assets,
        },
    ];

    const TIMELOCKED_ASSETS_CARDS: MigrationDisplayCard[] = [
        {
            title: `${totalUnmigratableObjects}`,
            subtitle: 'Time-locked',
            icon: Clock,
        },
    ];

    const selectedObjects = useMemo(() => {
        if (stardustMigrationObjects) {
            if (selectedStardustObjectsCategory === StardustOutputMigrationStatus.Migratable) {
                return [
                    ...stardustMigrationObjects.migratableBasicOutputs,
                    ...stardustMigrationObjects.migratableNftOutputs,
                ];
            } else if (
                selectedStardustObjectsCategory === StardustOutputMigrationStatus.TimeLocked
            ) {
                return [
                    ...stardustMigrationObjects.unmigratableBasicOutputs,
                    ...stardustMigrationObjects.unmigratableNftOutputs,
                ];
            }
        }
        return [];
    }, [selectedStardustObjectsCategory, stardustMigrationObjects]);

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

    function handleCloseDetailsPanel() {
        setSelectedStardustObjectsCategory(undefined);
    }

    return (
        <div className="flex h-full w-full flex-wrap items-center justify-center space-y-4">
            <div
                className={clsx(
                    'flex h-[700px] w-full flex-row items-stretch',
                    !selectedStardustObjectsCategory ? 'justify-center' : 'gap-md--rs',
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
                                    <CardBody
                                        title={isLoading ? '--' : card.title}
                                        subtitle={card.subtitle}
                                    />
                                </Card>
                            ))}
                            <Button
                                text="See All"
                                type={ButtonType.Ghost}
                                fullWidth
                                disabled={
                                    selectedStardustObjectsCategory ===
                                    StardustOutputMigrationStatus.Migratable
                                }
                                onClick={() =>
                                    setSelectedStardustObjectsCategory(
                                        StardustOutputMigrationStatus.Migratable,
                                    )
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
                                    <CardBody
                                        title={isLoading ? '--' : card.title}
                                        subtitle={card.subtitle}
                                    />
                                </Card>
                            ))}
                            <Button
                                text="See All"
                                type={ButtonType.Ghost}
                                fullWidth
                                disabled={
                                    selectedStardustObjectsCategory ===
                                    StardustOutputMigrationStatus.TimeLocked
                                }
                                onClick={() =>
                                    setSelectedStardustObjectsCategory(
                                        StardustOutputMigrationStatus.TimeLocked,
                                    )
                                }
                            />
                        </div>
                    </Panel>
                </div>

                <MigrationObjectsPanel
                    selectedObjects={selectedObjects}
                    onClose={handleCloseDetailsPanel}
                    isHidden={!selectedStardustObjectsCategory}
                    isTimelocked={
                        selectedStardustObjectsCategory === StardustOutputMigrationStatus.TimeLocked
                    }
                />
            </div>
        </div>
    );
}

export default MigrationDashboardPage;
