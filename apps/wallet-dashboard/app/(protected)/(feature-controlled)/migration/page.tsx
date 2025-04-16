// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useState, useCallback } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import clsx from 'clsx';
import { useGetStardustMigratableObjects, useGroupedStardustObjects } from '@/hooks';
import { getStardustObjectsTotals } from '@/lib/utils';
import {
    Address,
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
import { Assets, IotaLogoMark, Stake } from '@iota/apps-ui-icons';
import { useCurrentAccount, useIotaClient } from '@iota/dapp-kit';
import {
    STARDUST_BASIC_OUTPUT_TYPE,
    STARDUST_NFT_OUTPUT_TYPE,
    toast,
    addressToStardustBech32,
    useCopyToClipboard,
    useFormatCoin,
    useStardustIndexerClientContext,
} from '@iota/core';
import { StardustOutputMigrationStatus } from '@/lib/enums';
import { MigrationObjectsPanel, MigrationDialog } from '@/components';
import { useRouter } from 'next/navigation';

function MigrationDashboardPage(): JSX.Element {
    const account = useCurrentAccount();
    const address = account?.address || '';
    const bech32Address = addressToStardustBech32(address);
    const queryClient = useQueryClient();
    const iotaClient = useIotaClient();
    const router = useRouter();
    const [isMigrationDialogOpen, setIsMigrationDialogOpen] = useState(false);
    const [selectedStardustObjectsCategory, setSelectedStardustObjectsCategory] = useState<
        StardustOutputMigrationStatus | undefined
    >(undefined);
    const { stardustIndexerClient } = useStardustIndexerClientContext();
    const {
        migratableBasicOutputs,
        migratableNftOutputs,
        timelockedBasicOutputs,
        timelockedNftOutputs,
        isPending,
    } = useGetStardustMigratableObjects(address);

    const { data: resolvedMigrationObjects = [] } = useGroupedStardustObjects(
        [...(migratableBasicOutputs || []), ...(migratableNftOutputs || [])],
        false,
    );
    const { data: resolvedTimelockedObjects = [] } = useGroupedStardustObjects(
        [...(timelockedBasicOutputs || []), ...(timelockedNftOutputs || [])],
        true,
    );

    const {
        totalIotaAmount: migratableIotaAmount,
        totalNativeTokens: migratableNativeTokens,
        totalVisualAssets: migratableVisualAssets,
    } = getStardustObjectsTotals({
        basicOutputs: migratableBasicOutputs,
        nftOutputs: migratableNftOutputs,
        address,
        resolvedObjects: resolvedMigrationObjects,
    });
    const {
        totalIotaAmount: timelockedIotaAmount,
        totalNativeTokens: timelockedNativeTokens,
        totalVisualAssets: timelockedVisualAssets,
    } = getStardustObjectsTotals({
        basicOutputs: timelockedBasicOutputs,
        nftOutputs: timelockedNftOutputs,
        address,
        resolvedObjects: resolvedTimelockedObjects,
    });

    const hasMigratableObjects =
        (migratableBasicOutputs?.length || 0) > 0 || (migratableNftOutputs?.length || 0) > 0;
    const hasTimelockedObjects =
        (timelockedBasicOutputs?.length || 0) > 0 || (timelockedNftOutputs?.length || 0) > 0;

    const [migratableIotaAmountFormatted, migratableIotaAmountSymbol] = useFormatCoin({
        balance: migratableIotaAmount,
    });
    const [timelockedIotaAmountFormatted, timelockedIotaAmountSymbol] = useFormatCoin({
        balance: timelockedIotaAmount,
    });

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
                queryClient.invalidateQueries({
                    queryKey: ['migration-transaction', address],
                });
                queryClient.invalidateQueries({
                    queryKey: ['stardust-shared-objects', address, stardustIndexerClient],
                });
            });
        },
        [iotaClient, queryClient, address, stardustIndexerClient],
    );

    const MIGRATION_CARDS: MigrationDisplayCardProps[] = [
        {
            title: `${migratableIotaAmountFormatted} ${migratableIotaAmountSymbol}`,
            subtitle: 'IOTA Tokens',
            icon: IotaLogoMark,
        },
        {
            title: `${migratableNativeTokens}`,
            subtitle: 'Native Tokens',
            icon: Stake,
        },
        {
            title: `${migratableVisualAssets}`,
            subtitle: 'Visual Assets',
            icon: Assets,
        },
    ];

    const TIMELOCKED_ASSETS_CARDS: MigrationDisplayCardProps[] = [
        {
            title: `${timelockedIotaAmountFormatted} ${timelockedIotaAmountSymbol}`,
            subtitle: 'IOTA Tokens',
            icon: IotaLogoMark,
        },
        {
            title: `${timelockedNativeTokens}`,
            subtitle: 'Native Tokens',
            icon: Stake,
        },
        {
            title: `${timelockedVisualAssets}`,
            subtitle: 'Visual Assets',
            icon: Assets,
        },
    ];

    const selectedObjects =
        selectedStardustObjectsCategory === StardustOutputMigrationStatus.Migratable
            ? [...migratableBasicOutputs, ...migratableNftOutputs]
            : selectedStardustObjectsCategory === StardustOutputMigrationStatus.TimeLocked
              ? [...timelockedBasicOutputs, ...timelockedNftOutputs]
              : [];

    function openMigrationDialog(): void {
        setIsMigrationDialogOpen(true);
    }

    function handleCloseDetailsPanel() {
        setSelectedStardustObjectsCategory(undefined);
    }

    function handleMigrationDialogClose() {
        setIsMigrationDialogOpen(false);
        router.replace('/home');
    }

    const copyToClipBoard = useCopyToClipboard(() => toast('Address copied'));

    async function handleCopy() {
        await copyToClipBoard(bech32Address);
    }

    return (
        <div className="flex h-full w-full flex-wrap items-center justify-center space-y-4">
            <div
                className={clsx(
                    'flex h-full min-h-[700px] w-full flex-col items-stretch md:flex-row',
                    !selectedStardustObjectsCategory ? 'justify-center' : 'gap-md--rs',
                )}
            >
                <div
                    className={clsx(
                        'flex flex-col gap-md--rs',
                        !selectedStardustObjectsCategory ? 'w-full md:w-1/2' : 'w-full md:w-1/3',
                    )}
                >
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
                            handleClose={handleMigrationDialogClose}
                        />
                    )}
                    <Panel>
                        <div className="flex flex-col gap-y-xxxs break-all rounded-md px-md--rs py-sm--rs">
                            <span className="text-label-md">Stardust address</span>
                            <Address
                                text={bech32Address}
                                isExternal
                                externalLink={`https://explorer.iota.org/mainnet/addr/${bech32Address}`}
                                isCopyable
                                copyText={bech32Address}
                                onCopySuccess={handleCopy}
                            />
                        </div>
                    </Panel>
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
                        <div className="flex flex-col gap-xs p-sm--rs">
                            {MIGRATION_CARDS.map((card) => (
                                <MigrationDisplayCard
                                    key={card.subtitle}
                                    isPlaceholder={isPending}
                                    {...card}
                                />
                            ))}
                            <Button
                                text="See All"
                                type={ButtonType.Ghost}
                                fullWidth
                                disabled={
                                    selectedStardustObjectsCategory ===
                                        StardustOutputMigrationStatus.Migratable ||
                                    !hasMigratableObjects
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
                        <div className="flex flex-col gap-xs p-sm--rs">
                            {TIMELOCKED_ASSETS_CARDS.map((card) => (
                                <MigrationDisplayCard
                                    key={card.subtitle}
                                    isPlaceholder={isPending}
                                    {...card}
                                />
                            ))}
                            <Button
                                text="See All"
                                type={ButtonType.Ghost}
                                fullWidth
                                disabled={
                                    selectedStardustObjectsCategory ===
                                        StardustOutputMigrationStatus.TimeLocked ||
                                    !hasTimelockedObjects
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
                    groupByTimelockUC={
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
