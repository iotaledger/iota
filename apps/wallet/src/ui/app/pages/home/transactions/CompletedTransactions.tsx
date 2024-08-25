// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Alert, ErrorBoundary, Loading, TransactionCard, NoActivityCard } from '_components';
import { useQueryTransactionsByAddress } from '@iota/core';
import { useActiveAddress } from '_src/ui/app/hooks/useActiveAddress';

export function CompletedTransactions() {
    const activeAddress = useActiveAddress();
    const { data: txns, isPending, error } = useQueryTransactionsByAddress(activeAddress);
    if (error) {
        return <Alert>{(error as Error)?.message}</Alert>;
    }
    return (
        <Loading loading={isPending}>
            {txns?.length && activeAddress ? (
                txns.map((txn) => (
                    <ErrorBoundary key={txn.digest}>
                        <TransactionCard txn={txn} address={activeAddress} />
                    </ErrorBoundary>
                ))
            ) : (
                <NoActivityCard message="When available, your IOTA network transactions will show up here." />
            )}
        </Loading>
    );
}
