// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    Alert,
    ErrorBoundary,
    Loading,
    TransactionCard,
    NoData,
    AlertStyle,
    AlertType,
} from '_components';
import { useQueryTransactionsByAddress } from '@iota/core';
import { useActiveAddress } from '_src/ui/app/hooks/useActiveAddress';

export function CompletedTransactions() {
    const activeAddress = useActiveAddress();
    const { data: txns, isPending, error } = useQueryTransactionsByAddress(activeAddress);
    if (error) {
        return (
            <div className="mb-2 flex h-full w-full items-center justify-center p-2">
                <Alert
                    title="Something went worng"
                    supportingText={error?.message ?? 'An error occurred'}
                    style={AlertStyle.Default}
                    type={AlertType.Warning}
                />
            </div>
        );
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
                <NoData message="You can view your IOTA network transactions here once they are available." />
            )}
        </Loading>
    );
}
