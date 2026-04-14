// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Dialog } from '@iota/apps-ui-kit';
import { TransactionDialogView } from '../TransactionDialog';

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
            <TransactionDialogView txDigest={txDigest} onClose={onClose} />
        </Dialog>
    );
}
