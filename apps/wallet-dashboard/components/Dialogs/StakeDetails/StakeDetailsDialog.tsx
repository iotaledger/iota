// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DialogView } from '@/lib/interfaces';
import { StakeDetailsView } from './views';
import { useState } from 'react';
import { ExtendedDelegatedStake } from '@iota/core';
import { Dialog, DialogBody, DialogContent, DialogPosition, Header } from '@iota/apps-ui-kit';
import { UnstakeDialogView } from '../Unstake';

enum DialogViewIdentifier {
    StakeDetails = 'StakeDetails',
    Unstake = 'Unstake',
}

interface StakeDetailsDialogProps {
    extendedStake: ExtendedDelegatedStake;
    showActiveStatus?: boolean;
    handleClose: () => void;
}
export function StakeDetailsDialog({
    extendedStake,
    showActiveStatus,
    handleClose,
}: StakeDetailsDialogProps) {
    const [open, setOpen] = useState(true);

    const VIEWS: Record<DialogViewIdentifier, DialogView> = {
        [DialogViewIdentifier.StakeDetails]: {
            header: <Header title="Stake Details" onClose={handleClose} />,
            body: (
                <StakeDetailsView
                    extendedStake={extendedStake}
                    onUnstake={() => {
                        setCurrentView(VIEWS[DialogViewIdentifier.Unstake]);
                    }}
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

    const [currentView, setCurrentView] = useState<DialogView>(
        VIEWS[DialogViewIdentifier.StakeDetails],
    );

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
