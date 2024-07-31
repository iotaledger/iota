// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useActiveAccount } from '_src/ui/app/hooks/useActiveAccount';
import { useUnlockedGuard } from '_src/ui/app/hooks/useUnlockedGuard';
import PageTitle from '_src/ui/app/shared/PageTitle';
import { Navigate, useParams } from 'react-router-dom';

import { CompletedTransactions } from './CompletedTransactions';

function TransactionBlocksPage() {
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
        <div className="flex h-full flex-col flex-nowrap overflow-x-visible">
            <PageTitle title="Your Activity" />
            <div className="divide-gray-45 -mx-5 mt-5 flex-grow divide-x-0 divide-y divide-solid overflow-y-auto px-5">
                <CompletedTransactions />
            </div>
        </div>
    );
}

export default TransactionBlocksPage;
