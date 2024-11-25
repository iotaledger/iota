// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Dialog } from '@iota/apps-ui-kit';
import { DetailsView, SendView } from './views';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { AssetsDialogView } from './hooks';

interface AssetsDialogProps {
    isOpen: boolean;
    handleClose: () => void;
    asset: IotaObjectData | null;
    view: AssetsDialogView;
    setView: (view: AssetsDialogView) => void;
}

export function AssetsDialog({
    isOpen,
    handleClose,
    asset,
    setView,
    view,
}: AssetsDialogProps): JSX.Element {
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
            {view === AssetsDialogView.Send && <SendView />}
        </Dialog>
    );
}
