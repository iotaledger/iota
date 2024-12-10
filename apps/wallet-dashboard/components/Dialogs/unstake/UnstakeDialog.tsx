// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Dialog } from '@iota/apps-ui-kit';
import { UnstakeView } from '../Staking/views';
import { ExtendedDelegatedStake } from '@iota/core';
import { UnstakeTimelockedObjectsDialog } from '@/components';
import { TimelockedStakedObjectsGrouped } from '@/lib/utils';
import { UnstakeDialogView } from './enums';
import { IotaSignAndExecuteTransactionOutput } from '@iota/wallet-standard';

interface UnstakeDialogProps {
    view: UnstakeDialogView;
    handleClose: () => void;
    onSuccess: (tx: IotaSignAndExecuteTransactionOutput) => void;
    onBack?: (view: UnstakeDialogView) => (() => void) | undefined;
    groupedTimelockedObjects?: TimelockedStakedObjectsGrouped;
    extendedStake?: ExtendedDelegatedStake;
}

export function UnstakeDialog({
    view,
    handleClose,
    onSuccess,
    extendedStake,
    groupedTimelockedObjects,
    onBack,
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
                <UnstakeTimelockedObjectsDialog
                    onClose={handleClose}
                    groupedTimelockedObjects={groupedTimelockedObjects}
                    onBack={onBack?.(UnstakeDialogView.TimelockedUnstake)}
                    onSuccess={onSuccess}
                />
            )}
        </Dialog>
    );
}
