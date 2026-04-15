// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Dialog } from '@iota/apps-ui-kit';
import { TransactionDetailsLayout } from '../transaction';
import { DialogLayout } from '../layout';

interface CollectTransactionDialogProps {
    open: boolean;
    txDigest: string;
    onClose: () => void;
}

export function CollectTransactionDialog({
    open,
    txDigest,
    onClose,
}: CollectTransactionDialogProps): React.JSX.Element {
    return (
        <Dialog open={open} onOpenChange={(isOpen) => !isOpen && onClose()}>
            <DialogLayout>
                <TransactionDetailsLayout digest={txDigest} onClose={onClose} />
            </DialogLayout>
        </Dialog>
    );
}
