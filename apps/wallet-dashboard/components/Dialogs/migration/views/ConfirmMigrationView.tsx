// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { MigrationObjectLoading, VirtualList } from '@/components';
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
import { useGroupedMigrationObjectsByExpirationDate } from '@/hooks';
import { Loader, Warning } from '@iota/ui-icons';
import { Collapsible, useFormatCoin } from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { summarizeMigratableObjectValues } from '@/lib/utils';
import { MigrationObjectDetailsCard } from '@/components/migration/migration-object-details-card';
import { DialogLayout, DialogLayoutBody, DialogLayoutFooter } from '../../layout';
import { Transaction } from '@iota/iota-sdk/transactions';

interface ConfirmMigrationViewProps {
    basicOutputObjects: IotaObjectData[] | undefined;
    nftOutputObjects: IotaObjectData[] | undefined;
    onSuccess: () => void;
    setOpen: (bool: boolean) => void;
    isTimelocked: boolean;
    migrateData:
        | {
              transaction: Transaction;
              gasBudget: string | number | null;
          }
        | undefined;
    isMigrationPending: boolean;
    isMigrationError: boolean;
    isSendingTransaction: boolean;
}

export function ConfirmMigrationView({
    basicOutputObjects = [],
    nftOutputObjects = [],
    onSuccess,
    setOpen,
    isTimelocked,
    migrateData,
    isMigrationPending,
    isMigrationError,
    isSendingTransaction,
}: ConfirmMigrationViewProps): JSX.Element {
    const account = useCurrentAccount();

    const {
        data: resolvedObjects = [],
        isLoading,
        error: isGroupedMigrationError,
    } = useGroupedMigrationObjectsByExpirationDate(
        [...basicOutputObjects, ...nftOutputObjects],
        isTimelocked,
    );

    const { totalNotOwnedStorageDepositReturnAmount } = summarizeMigratableObjectValues({
        basicOutputs: basicOutputObjects,
        nftOutputs: nftOutputObjects,
        address: account?.address || '',
    });

    const [gasFee, gasFeeSymbol] = useFormatCoin(migrateData?.gasBudget, IOTA_TYPE_ARG);
    const [totalStorageDepositReturnAmountFormatted, totalStorageDepositReturnAmountSymbol] =
        useFormatCoin(totalNotOwnedStorageDepositReturnAmount.toString(), IOTA_TYPE_ARG);

    return (
        <DialogLayout>
            <Header title="Confirmation" onClose={() => setOpen(false)} titleCentered />
            <DialogLayoutBody>
                <div className="flex h-full flex-col gap-y-md overflow-y-auto">
                    {isGroupedMigrationError && !isLoading && (
                        <InfoBox
                            title="Error"
                            supportingText="Failed to load migration objects"
                            style={InfoBoxStyle.Elevated}
                            type={InfoBoxType.Error}
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
                            <Collapsible
                                defaultOpen
                                render={() => (
                                    <Title size={TitleSize.Small} title="Assets to Migrate" />
                                )}
                            >
                                <div className="h-[500px] pb-md--rs xl:h-[600px]">
                                    <VirtualList
                                        heightClassName="h-full"
                                        overflowClassName="overflow-y-auto"
                                        items={resolvedObjects}
                                        estimateSize={() => 58}
                                        render={(migrationObject) => (
                                            <MigrationObjectDetailsCard
                                                migrationObject={migrationObject}
                                                isTimelocked={isTimelocked}
                                            />
                                        )}
                                    />
                                </div>
                            </Collapsible>
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
