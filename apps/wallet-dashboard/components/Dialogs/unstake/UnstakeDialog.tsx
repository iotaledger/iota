// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Dialog } from '@iota/apps-ui-kit';
import { UnstakeView } from '../Staking/views';
import { ExtendedDelegatedStake } from '@iota/core';
import { UnstakeTimelockedObjectsDialog } from '@/components';
import { TimelockedStakedObjectsGrouped } from '@/lib/utils';
import { UnstakeDialogView } from './enums';
import { useNotifications, useUnstakeTransaction, UseUnstakeTransactionParams } from '@/hooks';
import { useCurrentAccount, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { IotaSignAndExecuteTransactionOutput } from '@iota/wallet-standard';
import { NotificationType } from '@/stores/notificationStore';

type DynamicUnstakeProps =
    | {
          extendedStake: ExtendedDelegatedStake;
          groupedTimelockedObjects?: never;
      }
    | {
          extendedStake?: never;
          groupedTimelockedObjects: TimelockedStakedObjectsGrouped;
      };

interface UnstakeDialogProps {
    view: UnstakeDialogView;
    handleClose: () => void;
    onSuccess: (tx: IotaSignAndExecuteTransactionOutput) => void;
}

export function UnstakeDialog({
    view,
    handleClose,
    onSuccess,
    extendedStake,
    groupedTimelockedObjects,
}: UnstakeDialogProps & DynamicUnstakeProps): React.JSX.Element {
    const activeAddress = useCurrentAccount()?.address ?? '';
    const { addNotification } = useNotifications();

    const unstakeTransaction: UseUnstakeTransactionParams = groupedTimelockedObjects
        ? {
              senderAddress: activeAddress,
              groupedTimelockedObjects,
          }
        : {
              senderAddress: activeAddress,
              stakedIotaId: extendedStake.stakedIotaId,
          };

    const { data: unstakeData, isPending: isUnstakeTxPending } =
        useUnstakeTransaction(unstakeTransaction);
    const { mutateAsync: signAndExecuteTransaction, isPending: isTransactionPending } =
        useSignAndExecuteTransaction();

    async function handleUnstake(): Promise<void> {
        if (!unstakeData) return;

        await signAndExecuteTransaction(
            {
                transaction: unstakeData.transaction,
            },
            {
                onSuccess: (tx) => {
                    onSuccess(tx);
                    handleClose();
                    addNotification('Unstake transaction has been sent');
                },
            },
        ).catch(() => {
            addNotification('Unstake transaction was not sent', NotificationType.Error);
        });
    }

    return (
        <Dialog open onOpenChange={handleClose}>
            {view === UnstakeDialogView.Unstake && extendedStake && (
                <UnstakeView
                    extendedStake={extendedStake}
                    handleClose={handleClose}
                    showActiveStatus
                    unstakeTx={unstakeData?.transaction}
                    handleUnstake={handleUnstake}
                    isUnstakePending={isUnstakeTxPending}
                    gasBudget={unstakeData?.gasBudget}
                />
            )}
            {view === UnstakeDialogView.TimelockedUnstake && groupedTimelockedObjects && (
                <UnstakeTimelockedObjectsDialog
                    onClose={handleClose}
                    groupedTimelockedObjects={groupedTimelockedObjects}
                    unstakeTx={unstakeData?.transaction}
                    handleUnstake={handleUnstake}
                    isUnstakeTxLoading={isUnstakeTxPending}
                    isTxPending={isTransactionPending}
                />
            )}
        </Dialog>
    );
}
