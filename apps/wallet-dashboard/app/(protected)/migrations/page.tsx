// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useState, useMemo, useCallback } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import clsx from 'clsx';
import MigratePopup from '@/components/Popup/Popups/MigratePopup';
import { useGetStardustMigratableObjects, usePopups } from '@/hooks';
import { summarizeMigratableObjectValues } from '@/lib/utils';
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
import { StardustObjectMigrationType } from '@/lib/enums';
import { MigrationObjectsPanel } from '@/components';

interface MigrationDisplayCard {
    title: string;
    subtitle: string;
    icon: React.FC;
}

export default function MigrationDashboardPage(): JSX.Element {
    const account = useCurrentAccount();
    const address = account?.address || '';
    const { openPopup, closePopup } = usePopups();
    const queryClient = useQueryClient();
    const iotaClient = useIotaClient();

    const [selectedStardustObjectsCategory, setSelectedStardustObjectsCategory] = useState<
        StardustObjectMigrationType | undefined
    >(undefined);

    const stardustMigrationObjects = useGetStardustMigratableObjects(address);
    const {
        migratableBasicOutputs,
        unmigratableBasicOutputs,
        migratableNftOutputs,
        unmigratableNftOutputs,
    } = stardustMigrationObjects;

    const { totalIotaAmount, totalNativeTokens, totalVisualAssets } = useMemo(
        () =>
            summarizeMigratableObjectValues({
                migratableBasicOutputs,
                migratableNftOutputs,
                address,
            }),
        [migratableBasicOutputs, migratableNftOutputs, address],
    );

    const [timelockedIotaTokens, symbol] = useFormatCoin(totalIotaAmount, IOTA_TYPE_ARG);
    const hasMigratableObjects =
        migratableBasicOutputs.length > 0 || migratableNftOutputs.length > 0;

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

    const openMigratePopup = useCallback(() => {
        openPopup(
            <MigratePopup
                basicOutputObjects={migratableBasicOutputs}
                nftOutputObjects={migratableNftOutputs}
                closePopup={closePopup}
                onSuccess={handleOnSuccess}
            />,
        );
    }, [migratableBasicOutputs, migratableNftOutputs, closePopup, openPopup, handleOnSuccess]);

    const MIGRATION_CARDS: MigrationDisplayCard[] = useMemo(
        () => [
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
        ],
        [timelockedIotaTokens, symbol, totalNativeTokens, totalVisualAssets],
    );

    const timelockedAssetsAmount = unmigratableBasicOutputs.length + unmigratableNftOutputs.length;
    const TIMELOCKED_ASSETS_CARDS: MigrationDisplayCard[] = useMemo(
        () => [
            {
                title: `${timelockedAssetsAmount}`,
                subtitle: 'Time-locked',
                icon: Clock,
            },
        ],
        [timelockedAssetsAmount],
    );

    const handleCloseDetailsPanel = useCallback(() => {
        setSelectedStardustObjectsCategory(undefined);
    }, []);

    const selectedObjects = useMemo(() => {
        if (selectedStardustObjectsCategory === StardustObjectMigrationType.Migration) {
            return [...migratableBasicOutputs, ...migratableNftOutputs];
        } else if (selectedStardustObjectsCategory === StardustObjectMigrationType.TimeLocked) {
            return [...unmigratableBasicOutputs, ...unmigratableNftOutputs];
        }
        return [];
    }, [
        selectedStardustObjectsCategory,
        migratableBasicOutputs,
        migratableNftOutputs,
        unmigratableBasicOutputs,
        unmigratableNftOutputs,
    ]);

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
                                    <CardBody title={card.title} subtitle={card.subtitle} />
                                </Card>
                            ))}
                            <Button
                                text="See All"
                                type={ButtonType.Ghost}
                                fullWidth
                                onClick={() =>
                                    setSelectedStardustObjectsCategory(
                                        StardustObjectMigrationType.Migration,
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
                                    <CardBody title={card.title} subtitle={card.subtitle} />
                                </Card>
                            ))}
                            <Button
                                text="See All"
                                type={ButtonType.Ghost}
                                fullWidth
                                onClick={() =>
                                    setSelectedStardustObjectsCategory(
                                        StardustObjectMigrationType.TimeLocked,
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
                />
            </div>
        </div>
    );
}
