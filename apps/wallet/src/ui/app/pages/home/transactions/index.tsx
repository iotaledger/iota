// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useActiveAccount, useUnlockedGuard } from '_hooks';
import { Navigate, useParams } from 'react-router-dom';
import { CompletedTransactions } from './CompletedTransactions';
import { PageTemplate } from '_src/ui/app/components';

export function TransactionBlocksPage() {
    const activeAccount = useActiveAccount();
    const { status } = useParams();
    const isPendingTransactions = status === 'pending';
    if (useUnlockedGuard()) {
        return null;
    }
    if (activeAccount && isPendingTransactions) {
        return <Navigate to="/transactions" replace />;
    }
    return (
        <PageTemplate title="Your Activity" isTitleCentered>
            <div className="flex h-full w-full flex-col items-center gap-xxxs">
                <CompletedTransactions />
            </div>
        </PageTemplate>
    );
}
