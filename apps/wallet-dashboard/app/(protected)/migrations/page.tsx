// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import MigratePopup from '@/components/Popup/Popups/MigratePopup';
import { usePopups } from '@/hooks';
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
import { useCurrentAccount, useIotaClient } from '@iota/dapp-kit';
import { STARDUST_BASIC_OUTPUT_TYPE, STARDUST_NFT_OUTPUT_TYPE, useFormatCoin } from '@iota/core';
import { useGetStardustMigratableObjects } from '@/hooks';
import { useQueryClient } from '@tanstack/react-query';
import { Assets, Clock, IotaLogoMark, Tokens } from '@iota/ui-icons';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

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

    const {
        migratableBasicOutputs,
        unmigratableBasicOutputs,
        migratableNftOutputs,
        unmigratableNftOutputs,
    } = useGetStardustMigratableObjects(address);

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

    return (
        <div className="flex h-full w-full flex-wrap items-center justify-center space-y-4">
            <div className="flex w-full flex-row justify-center">
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
                            <Button text="See All" type={ButtonType.Ghost} fullWidth />
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
                            <Button text="See All" type={ButtonType.Ghost} fullWidth />
                        </div>
                    </Panel>
                </div>
            </div>
        </div>
    );
}

export default MigrationDashboardPage;
