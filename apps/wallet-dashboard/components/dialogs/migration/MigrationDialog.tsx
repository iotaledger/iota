// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from 'react';
import { useCurrentAccount, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { useMigrationTransaction } from '@/hooks/useMigrationTransaction';
import { Dialog } from '@iota/apps-ui-kit';
import toast from 'react-hot-toast';
import { TransactionDialogView } from '../TransactionDialog';
import { MigrationDialogView } from './enums';
import { ConfirmMigrationView } from './views';
import { ampli } from '@/lib/utils/analytics';
import { SIZE_LIMIT_EXCEEDED } from '@iota/core';

// Number of objects to reduce on every attempt
const REDUCTION_STEP_SIZE = 25;

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
    const [basicOutputs, setBasicOutputs] = useState<IotaObjectData[]>(basicOutputObjects);
    const [nftOutputs, setNftOutputs] = useState<IotaObjectData[]>(nftOutputObjects);
    const [isPartialMigration, setIsPartialMigration] = useState<boolean>(false);
    const reductionSize = useRef(0);
    const [txDigest, setTxDigest] = useState<string | null>(null);
    const [view, setView] = useState<MigrationDialogView>(MigrationDialogView.Confirmation);

    const {
        data: migrateData,
        isPending: isMigrationPending,
        isError: isMigrationError,
        error,
    } = useMigrationTransaction(account?.address || '', basicOutputs, nftOutputs);
    const { mutateAsync: signAndExecuteTransaction, isPending: isSendingTransaction } =
        useSignAndExecuteTransaction();

    useEffect(() => {
        if (isMigrationError && error?.message.includes(SIZE_LIMIT_EXCEEDED)) {
            reductionSize.current += REDUCTION_STEP_SIZE;
            setBasicOutputs(basicOutputObjects.slice(0, -reductionSize.current));
            setNftOutputs(nftOutputObjects.slice(0, -reductionSize.current));
            setIsPartialMigration(true);
        }
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [isMigrationError]);

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
                    ampli.migration({
                        basicOutputObjects: basicOutputs.length,
                        nftOutputObjects: nftOutputs.length,
                    });
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
                    basicOutputObjects={basicOutputs}
                    nftOutputObjects={nftOutputs}
                    onSuccess={handleMigrate}
                    setOpen={setOpen}
                    groupByTimelockUC={isTimelocked}
                    migrateData={migrateData}
                    isMigrationPending={isMigrationPending}
                    isMigrationError={isMigrationError}
                    isPartialMigration={isPartialMigration}
                    isSendingTransaction={isSendingTransaction}
                />
            )}
            {view === MigrationDialogView.TransactionDetails && (
                <TransactionDialogView txDigest={txDigest} onClose={handleClose} />
            )}
        </Dialog>
    );
}
