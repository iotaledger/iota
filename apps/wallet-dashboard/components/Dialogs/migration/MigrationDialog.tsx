// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';
import { useCurrentAccount, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { useMigrationTransaction } from '@/hooks/useMigrationTransaction';
import { Dialog } from '@iota/apps-ui-kit';
import toast from 'react-hot-toast';
import { TransactionDialogView } from '../TransactionDialog';
import { MigrationDialogView } from './enums';
import { ConfirmMigrationView } from './views';

interface MigrationDialogProps {
    handleClose: () => void;
    basicOutputObjects: IotaObjectData[] | undefined;
    nftOutputObjects: IotaObjectData[] | undefined;
    onSuccess: (digest: string) => void;
    setOpen: (bool: boolean) => void;
    open: boolean;
    isTimelocked: boolean;
}

export function MigrationDialog({
    handleClose,
    basicOutputObjects = [],
    nftOutputObjects = [],
    onSuccess,
    open,
    setOpen,
    isTimelocked,
}: MigrationDialogProps): JSX.Element {
    const account = useCurrentAccount();
    const [txDigest, setTxDigest] = useState<string>('');
    const [view, setView] = useState<MigrationDialogView>(MigrationDialogView.Confirmation);

    const {
        data: migrateData,
        isPending: isMigrationPending,
        isError: isMigrationError,
    } = useMigrationTransaction(account?.address || '', basicOutputObjects, nftOutputObjects);

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
                    onSuccess(tx.digest);
                    setTxDigest(tx.digest);
                    setView(MigrationDialogView.TransactionDetails);
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
            {view === MigrationDialogView.Confirmation && (
                <ConfirmMigrationView
                    basicOutputObjects={basicOutputObjects}
                    nftOutputObjects={nftOutputObjects}
                    onSuccess={handleMigrate}
                    setOpen={setOpen}
                    isTimelocked={isTimelocked}
                    migrateData={migrateData}
                    isMigrationPending={isMigrationPending}
                    isMigrationError={isMigrationError}
                    isSendingTransaction={isSendingTransaction}
                />
            )}
            {view === MigrationDialogView.TransactionDetails && (
                <TransactionDialogView txDigest={txDigest} onClose={handleClose} />
            )}
        </Dialog>
    );
}
