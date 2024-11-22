// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Dialog } from '@iota/apps-ui-kit';
import { DetailsView } from './views';
import { IotaObjectData } from '@iota/iota-sdk/client';

export enum AssetsDialogView {
    Details,
    Send,
}

interface AssetsDialogProps {
    isOpen: boolean;
    handleClose: () => void;
    asset: IotaObjectData | null;
}

export function AssetsDialog({ isOpen, handleClose, asset }: AssetsDialogProps): JSX.Element {
    const [view, setView] = React.useState<AssetsDialogView>(AssetsDialogView.Details);

    function handleDetailsSend() {
        setView(AssetsDialogView.Send);
    }

    return (
        <Dialog open={isOpen} onOpenChange={() => handleClose()}>
            {view === AssetsDialogView.Details && asset && (
                <DetailsView
                    asset={asset}
                    handleClose={handleClose}
                    handleSend={handleDetailsSend}
                />
            )}
        </Dialog>
    );
}
