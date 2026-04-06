// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ErrorBoundary, TransactionBlocksForAddress } from '~/components';

interface TransactionsViewProps {
    objectId: string;
}

export function TransactionsView({ objectId }: TransactionsViewProps) {
    return (
        <ErrorBoundary>
            <TransactionBlocksForAddress address={objectId} header="Transaction Blocks" />
        </ErrorBoundary>
    );
}
