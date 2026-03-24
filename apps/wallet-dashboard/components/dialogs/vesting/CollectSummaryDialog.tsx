// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { Dialog, DialogBody, DialogContent, Header } from '@iota/apps-ui-kit';
import { CollectSummary } from './CollectSummary';

interface CollectSummaryDialogProps {
    open: boolean;
    onClose: () => void;
    transaction: IotaTransactionBlockResponse;
    activeAddress: string;
}

export function CollectSummaryDialog({
    open,
    onClose,
    transaction,
    activeAddress,
}: CollectSummaryDialogProps) {
    return (
        <Dialog open={open} onOpenChange={(isOpen) => !isOpen && onClose()}>
            <DialogContent>
                <Header title="Collection Summary" onClose={onClose} />
                <DialogBody>
                    <CollectSummary transaction={transaction} activeAddress={activeAddress} />
                </DialogBody>
            </DialogContent>
        </Dialog>
    );
}
