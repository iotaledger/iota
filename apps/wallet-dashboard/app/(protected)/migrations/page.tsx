// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useState, useMemo, useCallback } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import clsx from 'clsx';
import { useGetStardustMigratableObjects } from '@/hooks';
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
import MigrationDialog from '@/components/Dialogs/MigrationDialog';

function MigrationDashboardPage(): JSX.Element {
    const account = useCurrentAccount();
    const address = account?.address || '';
    const queryClient = useQueryClient();
    const iotaClient = useIotaClient();
    const [isMigrationDialogOpen, setIsMigrationDialogOpen] = useState(false);
    const [selectedStardustObjectsCategory, setSelectedStardustObjectsCategory] = useState<
        StardustOutputMigrationStatus | undefined
    >(undefined);

    const { data: stardustMigrationObjects, isPlaceholderData } =
        useGetStardustMigratableObjects(address);
    const {
        migratableBasicOutputs,
        migratableNftOutputs,
        unmigratableBasicOutputs,
        unmigratableNftOutputs,
    } = stardustMigrationObjects || {};

    const {
        totalIotaAmount,
        totalNativeTokens: migratableNativeTokens,
        totalVisualAssets: migratableVisualAssets,
    } = summarizeMigratableObjectValues({
        basicOutputs: migratableBasicOutputs,
        nftOutputs: migratableNftOutputs,
        address,
    });
    const { totalUnmigratableObjects } = summarizeUnmigratableObjectValues({
        basicOutputs: unmigratableBasicOutputs,
        nftOutputs: unmigratableNftOutputs,
    });

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

    const MIGRATION_CARDS: MigrationDisplayCardProps[] = [
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

    const TIMELOCKED_ASSETS_CARDS: MigrationDisplayCardProps[] = [
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

    function openMigrationDialog(): void {
        setIsMigrationDialogOpen(true);
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
                    {isMigrationDialogOpen && (
                        <MigrationDialog
                            basicOutputObjects={migratableBasicOutputs}
                            nftOutputObjects={migratableNftOutputs}
                            onSuccess={handleOnSuccess}
                            open={isMigrationDialogOpen}
                            setOpen={setIsMigrationDialogOpen}
                            isTimelocked={
                                selectedStardustObjectsCategory ===
                                StardustOutputMigrationStatus.TimeLocked
                            }
                        />
                    )}
                    <Panel>
                        <Title
                            title="Migration"
                            trailingElement={
                                <Button
                                    text="Migrate All"
                                    disabled={!hasMigratableObjects}
                                    onClick={openMigrationDialog}
                                    size={ButtonSize.Small}
                                />
                            }
                        />
                        <div className="flex flex-col gap-xs p-md--rs">
                            {MIGRATION_CARDS.map((card) => (
                                <MigrationDisplayCard
                                    key={card.subtitle}
                                    isPlaceholder={isPlaceholderData}
                                    {...card}
                                />
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
                                <MigrationDisplayCard
                                    key={card.subtitle}
                                    isPlaceholder={isPlaceholderData}
                                    {...card}
                                />
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
                    isTimelocked={
                        selectedStardustObjectsCategory === StardustOutputMigrationStatus.TimeLocked
                    }
                />
            </div>
        </div>
    );
}

interface MigrationDisplayCardProps {
    title: string;
    subtitle: string;
    icon: React.ComponentType;
    isPlaceholder?: boolean;
}

function MigrationDisplayCard({
    title,
    subtitle,
    icon: Icon,
    isPlaceholder,
}: MigrationDisplayCardProps): React.JSX.Element {
    return (
        <Card>
            <CardImage shape={ImageShape.SquareRounded}>
                <Icon />
            </CardImage>
            <CardBody title={isPlaceholder ? '--' : title} subtitle={subtitle} />
        </Card>
    );
}

export default MigrationDashboardPage;
