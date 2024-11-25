// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { VirtualList } from '@/components';
import {
    useCurrentAccount,
    useIotaClientContext,
    useSignAndExecuteTransaction,
} from '@iota/dapp-kit';
import { getNetwork, IotaObjectData } from '@iota/iota-sdk/client';
import { useMigrationTransaction } from '@/hooks/useMigrationTransaction';
import { Button, InfoBox, InfoBoxStyle, InfoBoxType } from '@iota/apps-ui-kit';
import { useNotifications } from '@/hooks';
import { NotificationType } from '@/stores/notificationStore';
import { Loader, Warning } from '@iota/ui-icons';

interface MigratePopupProps {
    basicOutputObjects: IotaObjectData[];
    nftOutputObjects: IotaObjectData[];
    closePopup: () => void;
    onSuccess?: (digest: string) => void;
}

function MigratePopup({
    basicOutputObjects = [],
    nftOutputObjects = [],
    closePopup,
    onSuccess,
}: MigratePopupProps): JSX.Element {
    const account = useCurrentAccount();
    const { addNotification } = useNotifications();
    const {
        data: migrateData,
        isPending,
        isError,
        error,
    } = useMigrationTransaction(account?.address || '', basicOutputObjects, nftOutputObjects);

    const { network } = useIotaClientContext();
    const { explorer } = getNetwork(network);
    const { mutateAsync: signAndExecuteTransaction, isPending: isSendingTransaction } =
        useSignAndExecuteTransaction();

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
                closePopup();
                addNotification('Migration transaction has been sent');
            })
            .catch(() => {
                addNotification('Migration transaction was not sent', NotificationType.Error);
            });
    }

    const virtualItem = (asset: IotaObjectData): JSX.Element => (
        <a href={`${explorer}/object/${asset.objectId}`} target="_blank" rel="noreferrer">
            {asset.objectId}
        </a>
    );
    return (
        <div className="flex min-w-[300px] flex-col gap-2">
            <div className="flex flex-col">
                <h1>Migratable Basic Outputs: {basicOutputObjects?.length}</h1>
                <VirtualList
                    items={basicOutputObjects ?? []}
                    estimateSize={() => 30}
                    render={virtualItem}
                />
            </div>
            <div className="flex flex-col">
                <h1>Migratable Nft Outputs: {nftOutputObjects?.length}</h1>
                <VirtualList
                    items={nftOutputObjects ?? []}
                    estimateSize={() => 30}
                    render={virtualItem}
                />
            </div>
            <p>Gas Fees: {migrateData?.gasBudget?.toString() || '--'}</p>
            {isError ? (
                <InfoBox
                    type={InfoBoxType.Error}
                    title={error?.message || 'Error creating migration transcation'}
                    icon={<Warning />}
                    style={InfoBoxStyle.Elevated}
                />
            ) : null}
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
            />
        </div>
    );
}

export default MigratePopup;
