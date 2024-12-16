// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { VirtualList } from '@/components';
import { useCurrentAccount, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { useMigrationTransaction } from '@/hooks/useMigrationTransaction';
import { Button, Dialog, Header, KeyValueInfo, Panel } from '@iota/apps-ui-kit';
import { useGroupedMigrationObjectsByExpirationDate, useNotifications } from '@/hooks';
import { NotificationType } from '@/stores/notificationStore';
import { Loader } from '@iota/ui-icons';
import { DialogLayout, DialogLayoutBody, DialogLayoutFooter } from './layout';
import { MigrationObjectDetailsCard } from '../migration/migration-object-details-card';
import { useFormatCoin } from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { summarizeMigratableObjectValues } from '@/lib/utils';

interface MigrationDialogProps {
    basicOutputObjects: IotaObjectData[] | undefined;
    nftOutputObjects: IotaObjectData[] | undefined;
    onSuccess?: (digest: string) => void;
    setOpen: (bool: boolean) => void;
    open: boolean;
    isTimelocked: boolean;
}

function MigrationDialog({
    basicOutputObjects = [],
    nftOutputObjects = [],
    onSuccess,
    open,
    setOpen,
    isTimelocked,
}: MigrationDialogProps): JSX.Element {
    const account = useCurrentAccount();
    const { addNotification } = useNotifications();
    const {
        data: migrateData,
        isPending,
        isError,
        error,
    } = useMigrationTransaction(account?.address || '', basicOutputObjects, nftOutputObjects);
    const [gasFee, gasFeesymbol] = useFormatCoin(migrateData?.gasBudget, IOTA_TYPE_ARG);

    const { mutateAsync: signAndExecuteTransaction, isPending: isSendingTransaction } =
        useSignAndExecuteTransaction();

    const { totalIotaAmount } = summarizeMigratableObjectValues({
        basicOutputs: basicOutputObjects,
        nftOutputs: nftOutputObjects,
        address: account?.address || '',
    });
    const [totalIotaAmountFormatted, totalIotaAmountSymbol] = useFormatCoin(
        totalIotaAmount.toString(),
        IOTA_TYPE_ARG,
    );

    async function handleMigrate(): Promise<void> {
        if (!migrateData) return;
        signAndExecuteTransaction(
            {
                transaction: migrateData.transaction,
            },
            {
                onSuccess: (tx) => {
                    if (onSuccess) {
                        onSuccess(tx.digest);
                    }
                },
            },
        )
            .then(() => {
                addNotification('Migration transaction has been sent');
            })
            .catch(() => {
                addNotification('Migration transaction was not sent', NotificationType.Error);
            });
    }

    const {
        data: resolvedObjects = [],
        isLoading,
        error: isErrored,
    } = useGroupedMigrationObjectsByExpirationDate(
        [...basicOutputObjects, ...nftOutputObjects],
        isTimelocked,
    );

    return (
        <Dialog open={open} onOpenChange={setOpen}>
            <DialogLayout>
                <Header title="Confirmation" onClose={() => setOpen(false)} titleCentered />
                <DialogLayoutBody>
                    <div className="flex h-full flex-col gap-y-md">
                        <VirtualList
                            heightClassName="h-[600px]"
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
                        <Panel hasBorder>
                            <div className="flex flex-col gap-y-sm p-md">
                                <KeyValueInfo
                                    keyText="Legacy storage deposit"
                                    value={totalIotaAmountFormatted || '-'}
                                    supportingLabel={totalIotaAmountSymbol}
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
                    </div>
                </DialogLayoutBody>
                <DialogLayoutFooter>
                    <Button
                        text="Migrate"
                        disabled={isPending || isError || isSendingTransaction}
                        onClick={handleMigrate}
                        icon={
                            isPending || isSendingTransaction ? (
                                <Loader className="h-4 w-4 animate-spin" />
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

export default MigrationDialog;
