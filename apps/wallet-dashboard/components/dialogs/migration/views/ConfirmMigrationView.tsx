// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { MigrationObjectLoading, MigrationObjectDetailsCard } from '@/components';
import { useCurrentAccount } from '@iota/dapp-kit';
import { IotaObjectData } from '@iota/iota-sdk/client';
import {
    Button,
    Header,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    KeyValueInfo,
    Panel,
    Skeleton,
    Title,
    TitleSize,
} from '@iota/apps-ui-kit';
import { useGroupedStardustObjects } from '@/hooks';
import { Exclamation, Loader, Warning } from '@iota/apps-ui-icons';
import {
    CoinFormat,
    Collapsible,
    GAS_BUDGET_ERROR_MESSAGES,
    GAS_BALANCE_TOO_LOW_ID,
    GasSummaryType,
    useBalance,
    useFormatCoin,
    VirtualList,
} from '@iota/core';
import { getStardustObjectsTotals, filterMigrationObjects } from '@/lib/utils';
import { DialogLayout, DialogLayoutBody, DialogLayoutFooter } from '../../layout';
import { Transaction } from '@iota/iota-sdk/transactions';
import { StardustOutputDetailsFilter } from '@/lib/enums';

interface ConfirmMigrationViewProps {
    basicOutputObjects: IotaObjectData[] | undefined;
    nftOutputObjects: IotaObjectData[] | undefined;
    onSuccess: () => void;
    setOpen: (bool: boolean) => void;
    groupByTimelockUC: boolean;
    migrateData:
        | {
              transaction: Transaction;
              gasSummary: GasSummaryType;
          }
        | undefined;
    isMigrationPending: boolean;
    isMigrationError: boolean;
    isPartialMigration: boolean;
    isSendingTransaction: boolean;
}

export function ConfirmMigrationView({
    basicOutputObjects = [],
    nftOutputObjects = [],
    onSuccess,
    setOpen,
    groupByTimelockUC,
    migrateData,
    isMigrationPending,
    isMigrationError,
    isPartialMigration,
    isSendingTransaction,
}: ConfirmMigrationViewProps): JSX.Element {
    const account = useCurrentAccount();
    const { data: balance, isLoading: isLoadingBalance } = useBalance(account?.address || '');
    const hasBalance = BigInt(balance?.totalBalance || 0) > BigInt(0);

    const {
        data: resolvedObjects = [],
        isLoading,
        error: isGroupedMigrationError,
    } = useGroupedStardustObjects([...basicOutputObjects, ...nftOutputObjects], groupByTimelockUC);

    const {
        totalIotaAmount,
        totalNativeTokens: migratableNativeTokens,
        totalVisualAssets: migratableVisualAssets,
        totalNotOwnedStorageDepositReturnAmount,
    } = getStardustObjectsTotals({
        basicOutputs: basicOutputObjects,
        nftOutputs: nftOutputObjects,
        address: account?.address || '',
        resolvedObjects: resolvedObjects,
    });

    const [gasFee, gasFeeSymbol] = useFormatCoin({
        balance: migrateData?.gasSummary?.totalGas,
        format: CoinFormat.FULL,
    });
    const [timelockedIotaTokens, symbol] = useFormatCoin({ balance: totalIotaAmount });
    const [totalStorageDepositReturnAmountFormatted, totalStorageDepositReturnAmountSymbol] =
        useFormatCoin({ balance: totalNotOwnedStorageDepositReturnAmount.toString() });

    const filteredIotaObjects = filterMigrationObjects(
        resolvedObjects,
        StardustOutputDetailsFilter.IOTA,
    );
    const filteredNativeTokens = filterMigrationObjects(
        resolvedObjects,
        StardustOutputDetailsFilter.NativeTokens,
    );
    const filteredVisualAssets = filterMigrationObjects(
        resolvedObjects,
        StardustOutputDetailsFilter.VisualAssets,
    );

    const hasEnoughBalanceToPayGas =
        BigInt(balance?.totalBalance || 0) >= BigInt(migrateData?.gasSummary?.totalGas || 0);

    const assetsToMigrateCategories = [
        {
            title: 'IOTA Tokens',
            subtitle: `${timelockedIotaTokens} ${symbol}`,
            filteredObjects: filteredIotaObjects,
        },
        {
            title: 'Native Tokens',
            subtitle: `${migratableNativeTokens} Types`,
            filteredObjects: filteredNativeTokens,
        },
        {
            title: 'Visual Assets',
            subtitle: `${migratableVisualAssets} Assets`,
            filteredObjects: filteredVisualAssets,
        },
    ];
    const filteredAssetsToMigrateCategories = assetsToMigrateCategories.filter(
        ({ filteredObjects }) => filteredObjects.length > 0,
    );
    return (
        <DialogLayout>
            <Header title="Migrate Your Assets" onClose={() => setOpen(false)} titleCentered />
            <DialogLayoutBody>
                <div className="flex h-full flex-col gap-y-md">
                    {!isLoadingBalance && !hasBalance && (
                        <InfoBox
                            title="Insufficient balance"
                            supportingText="You don't have enough balance to migrate"
                            style={InfoBoxStyle.Elevated}
                            type={InfoBoxType.Error}
                            icon={<Warning />}
                        />
                    )}
                    {isGroupedMigrationError && !isLoading && (
                        <InfoBox
                            title="Error"
                            supportingText="Failed to load migration objects"
                            style={InfoBoxStyle.Elevated}
                            type={InfoBoxType.Error}
                            icon={<Warning />}
                        />
                    )}
                    {isPartialMigration && !isLoading && (
                        <InfoBox
                            title="Partial migration"
                            supportingText="Due to the large number of objects, a partial migration will be attempted. After the migration is complete, you can migrate the remaining assets."
                            style={InfoBoxStyle.Elevated}
                            type={InfoBoxType.Warning}
                            icon={<Warning />}
                        />
                    )}
                    {isLoading ? (
                        <>
                            <Panel hasBorder>
                                <div className="flex flex-col gap-y-sm p-md">
                                    <Skeleton widthClass="w-40" heightClass="h-3.5" />
                                    <MigrationObjectLoading />
                                </div>
                            </Panel>
                            <Panel hasBorder>
                                <div className="flex flex-col gap-y-md p-md">
                                    <Skeleton widthClass="w-full" heightClass="h-3.5" />
                                    <Skeleton widthClass="w-full" heightClass="h-3.5" />
                                </div>
                            </Panel>
                        </>
                    ) : (
                        <>
                            <div className="flex flex-col gap-y-sm">
                                {filteredAssetsToMigrateCategories.map(
                                    ({ title, subtitle, filteredObjects }) => (
                                        <Collapsible
                                            key={title}
                                            render={() => (
                                                <Title
                                                    size={TitleSize.Small}
                                                    title={title}
                                                    subtitle={subtitle}
                                                />
                                            )}
                                        >
                                            <div className="flex h-full max-h-[300px] flex-col gap-y-sm pb-sm">
                                                <VirtualList
                                                    heightClassName="h-full"
                                                    items={filteredObjects}
                                                    getItemKey={(migrationObject) =>
                                                        migrationObject.uniqueId
                                                    }
                                                    estimateSize={() => 58}
                                                    render={(migrationObject) => (
                                                        <MigrationObjectDetailsCard
                                                            migrationObject={migrationObject}
                                                            isTimelocked={groupByTimelockUC}
                                                        />
                                                    )}
                                                />
                                            </div>
                                        </Collapsible>
                                    ),
                                )}
                            </div>
                            <Panel hasBorder>
                                <div className="flex flex-col gap-y-sm p-md">
                                    <KeyValueInfo
                                        keyText="Legacy storage deposit"
                                        value={totalStorageDepositReturnAmountFormatted || '-'}
                                        supportingLabel={totalStorageDepositReturnAmountSymbol}
                                        fullwidth
                                    />
                                    <KeyValueInfo
                                        keyText="Gas Fees"
                                        value={gasFee || '-'}
                                        supportingLabel={gasFeeSymbol}
                                        fullwidth
                                    />
                                </div>
                            </Panel>
                        </>
                    )}
                </div>
            </DialogLayoutBody>
            <DialogLayoutFooter>
                {!hasEnoughBalanceToPayGas && (
                    <div className="mb-sm">
                        <InfoBox
                            type={InfoBoxType.Error}
                            supportingText={GAS_BUDGET_ERROR_MESSAGES[GAS_BALANCE_TOO_LOW_ID]}
                            style={InfoBoxStyle.Elevated}
                            icon={<Exclamation />}
                        />
                    </div>
                )}
                <Button
                    text="Migrate"
                    disabled={isMigrationPending || isMigrationError || isSendingTransaction}
                    onClick={onSuccess}
                    icon={
                        isMigrationPending || isSendingTransaction ? (
                            <Loader
                                className="h-4 w-4 animate-spin"
                                data-testid="loading-indicator"
                            />
                        ) : null
                    }
                    iconAfterText
                    fullWidth
                />
            </DialogLayoutFooter>
        </DialogLayout>
    );
}
