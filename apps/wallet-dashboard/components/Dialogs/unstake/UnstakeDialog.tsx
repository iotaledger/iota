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

type UnstakeByTypeProps =
    | {
          extendedStake: ExtendedDelegatedStake;
          groupedTimelockedObjects?: never;
      }
    | {
          groupedTimelockedObjects: TimelockedStakedObjectsGrouped;
          extendedStake?: never;
      };

interface UnstakeDialogProps {
    view: UnstakeDialogView;
    handleClose: () => void;
    onSuccess: (tx: IotaSignAndExecuteTransactionOutput) => void;
    onBack?: (view: UnstakeDialogView) => (() => void) | undefined;
}

export function UnstakeDialog({
    view,
    handleClose,
    onSuccess,
    extendedStake,
    groupedTimelockedObjects,
    onBack,
}: UnstakeDialogProps & UnstakeByTypeProps): React.JSX.Element {
    const activeAddress = useCurrentAccount()?.address ?? '';
    const { addNotification } = useNotifications();

    const unstakeParams: UseUnstakeTransactionParams = groupedTimelockedObjects
        ? {
              senderAddress: activeAddress,
              unstakeIotaIds: groupedTimelockedObjects.stakes.map(
                  (stake) => stake.timelockedStakedIotaId,
              ),
              isTimelockedUnstake: true,
          }
        : {
              senderAddress: activeAddress,
              unstakeIotaId: extendedStake.stakedIotaId,
          };

    const { data: unstakeData, isPending: isUnstakeTxPending } =
        useUnstakeTransaction(unstakeParams);
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
                    onBack={onBack?.(UnstakeDialogView.Unstake)}
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
                    onBack={onBack?.(UnstakeDialogView.TimelockedUnstake)}
                    isUnstakeTxLoading={isUnstakeTxPending}
                    isTxPending={isTransactionPending}
                />
            )}
        </Dialog>
    );
}
