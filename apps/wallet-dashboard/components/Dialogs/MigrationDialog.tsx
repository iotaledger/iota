// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { VirtualList } from '@/components';
import { useCurrentAccount, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { useMigrationTransaction } from '@/hooks/useMigrationTransaction';
import {
    Button,
    Dialog,
    Header,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    KeyValueInfo,
    LoadingIndicator,
    Panel,
    Title,
    TitleSize,
} from '@iota/apps-ui-kit';
import { useGroupedMigrationObjectsByExpirationDate } from '@/hooks';
import { Loader, Warning } from '@iota/ui-icons';
import { DialogLayout, DialogLayoutBody, DialogLayoutFooter } from './layout';
import { MigrationObjectDetailsCard } from '../migration/migration-object-details-card';
import { Collapsible, useFormatCoin } from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { summarizeMigratableObjectValues } from '@/lib/utils';
import toast from 'react-hot-toast';

interface MigrationDialogProps {
    basicOutputObjects: IotaObjectData[] | undefined;
    nftOutputObjects: IotaObjectData[] | undefined;
    onSuccess: (digest: string) => void;
    setOpen: (bool: boolean) => void;
    open: boolean;
    isTimelocked: boolean;
}

export function MigrationDialog({
    basicOutputObjects = [],
    nftOutputObjects = [],
    onSuccess,
    open,
    setOpen,
    isTimelocked,
}: MigrationDialogProps): JSX.Element {
    const account = useCurrentAccount();
    const {
        data: migrateData,
        isPending: isMigrationPending,
        isError: isMigrationError,
    } = useMigrationTransaction(account?.address || '', basicOutputObjects, nftOutputObjects);

    const {
        data: resolvedObjects = [],
        isLoading,
        error: isGroupedMigrationError,
    } = useGroupedMigrationObjectsByExpirationDate(
        [...basicOutputObjects, ...nftOutputObjects],
        isTimelocked,
    );

    const { mutateAsync: signAndExecuteTransaction, isPending: isSendingTransaction } =
        useSignAndExecuteTransaction();
    const { totalNonOwnedStorageDepositReturnAmount } = summarizeMigratableObjectValues({
        basicOutputs: basicOutputObjects,
        nftOutputs: nftOutputObjects,
        address: account?.address || '',
    });

    const [gasFee, gasFeesymbol] = useFormatCoin(migrateData?.gasBudget, IOTA_TYPE_ARG);
    const [
        totaltotalStorageDepositReturnAmountFormatted,
        totaltotalStorageDepositReturnAmountSymbol,
    ] = useFormatCoin(totalNonOwnedStorageDepositReturnAmount.toString(), IOTA_TYPE_ARG);

    async function handleMigrate(): Promise<void> {
        if (!migrateData) return;
        signAndExecuteTransaction(
            {
                transaction: migrateData.transaction,
            },
            {
                onSuccess: (tx) => {
                    onSuccess(tx.digest);
                },
            },
        )
            .then(() => {
                toast.success('Migration transaction has been sent');
            })
            .catch(() => {
                toast.error('Migration transaction was not sent');
            });
    }

    return (
        <Dialog open={open} onOpenChange={setOpen}>
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
                            <LoadingIndicator text="Loading migration objects" />
                        ) : (
                            <>
                                <Collapsible
                                    defaultOpen
                                    render={() => (
                                        <Title size={TitleSize.Small} title="Assets to Migrate" />
                                    )}
                                >
                                    <div className="h-[600px] pb-md--rs">
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
                                            value={
                                                totaltotalStorageDepositReturnAmountFormatted || '-'
                                            }
                                            supportingLabel={
                                                totaltotalStorageDepositReturnAmountSymbol
                                            }
                                            fullwidth
                                        />
                                        <KeyValueInfo
                                            keyText="Gas Fees"
                                            value={gasFee || '-'}
                                            supportingLabel={gasFeesymbol}
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
                        onClick={handleMigrate}
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
        </Dialog>
    );
}
