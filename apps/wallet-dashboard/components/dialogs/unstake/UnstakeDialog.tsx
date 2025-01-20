// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Dialog } from '@iota/apps-ui-kit';
import { UnstakeTimelockedObjectsView, UnstakeView } from './views';
import { ExtendedDelegatedStake } from '@iota/core';
import { TimelockedStakedObjectsGrouped } from '@/lib/utils';
import { UnstakeDialogView } from './enums';
import { IotaSignAndExecuteTransactionOutput } from '@iota/wallet-standard';
import { TransactionDialogView } from '../TransactionDialog';

interface UnstakeDialogProps {
    view: UnstakeDialogView;
    handleClose: () => void;
    onSuccess: (tx: IotaSignAndExecuteTransactionOutput) => void;
    onBack?: (view: UnstakeDialogView) => (() => void) | undefined;
    groupedTimelockedObjects?: TimelockedStakedObjectsGrouped;
    extendedStake?: ExtendedDelegatedStake;
    txDigest?: string;
}

export function UnstakeDialog({
    view,
    handleClose,
    onSuccess,
    extendedStake,
    groupedTimelockedObjects,
    onBack,
    txDigest,
}: UnstakeDialogProps): React.JSX.Element {
    return (
        <Dialog open onOpenChange={handleClose}>
            {view === UnstakeDialogView.Unstake && extendedStake && (
                <UnstakeView
                    extendedStake={extendedStake}
                    handleClose={handleClose}
                    onBack={onBack?.(UnstakeDialogView.Unstake)}
                    showActiveStatus
                    onSuccess={onSuccess}
                />
            )}

            {view === UnstakeDialogView.TimelockedUnstake && groupedTimelockedObjects && (
                <UnstakeTimelockedObjectsView
                    onClose={handleClose}
                    groupedTimelockedObjects={groupedTimelockedObjects}
                    onBack={onBack?.(UnstakeDialogView.TimelockedUnstake)}
                    onSuccess={onSuccess}
                />
            )}
            {view === UnstakeDialogView.TransactionDetails && (
                <TransactionDialogView txDigest={txDigest} onClose={handleClose} />
            )}
        </Dialog>
    );
}
