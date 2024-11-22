// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DialogView } from '@/lib/interfaces';
import { StakeDialogView } from './views';
import { useState } from 'react';
import { ExtendedDelegatedStake } from '@iota/core';
import { Dialog, DialogBody, DialogContent, DialogPosition, Header } from '@iota/apps-ui-kit';
import { UnstakeDialogView } from '../Unstake';

enum DialogViewIdentifier {
    StakeDetails = 'StakeDetails',
    Unstake = 'Unstake',
}

interface StakeDetailsProps {
    extendedStake: ExtendedDelegatedStake;
    showActiveStatus?: boolean;
    handleClose: () => void;
}

export function StakeDetailsDialog({
    extendedStake,
    showActiveStatus,
    handleClose,
}: StakeDetailsProps) {
    const [open, setOpen] = useState(true);
    const [currentViewId, setCurrentViewId] = useState<DialogViewIdentifier>(
        DialogViewIdentifier.StakeDetails,
    );

    const VIEWS: Record<DialogViewIdentifier, DialogView> = {
        [DialogViewIdentifier.StakeDetails]: {
            header: <Header title="Stake Details" onClose={handleClose} />,
            body: (
                <StakeDialogView
                    extendedStake={extendedStake}
                    onUnstake={() => setCurrentViewId(DialogViewIdentifier.Unstake)}
                />
            ),
        },
        [DialogViewIdentifier.Unstake]: {
            header: <Header title="Unstake" onClose={handleClose} />,
            body: (
                <UnstakeDialogView
                    extendedStake={extendedStake}
                    handleClose={handleClose}
                    showActiveStatus={showActiveStatus}
                />
            ),
        },
    };

    const currentView = VIEWS[currentViewId];

    return (
        <Dialog
            open={open}
            onOpenChange={(open) => {
                if (!open) {
                    handleClose();
                }
                setOpen(open);
            }}
        >
            <DialogContent containerId="overlay-portal-container" position={DialogPosition.Right}>
                {currentView.header}
                <div className="flex h-full [&>div]:flex [&>div]:flex-1 [&>div]:flex-col">
                    <DialogBody>{currentView.body}</DialogBody>
                </div>
            </DialogContent>
        </Dialog>
    );
}
