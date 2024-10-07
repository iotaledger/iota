// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ErrorBoundary, Loading, TransactionCard, NoData } from '_components';
import { useQueryTransactionsByAddress } from '@iota/core';
import { useActiveAddress } from '_src/ui/app/hooks/useActiveAddress';
import { InfoBox, InfoBoxStyle, InfoBoxType } from '@iota/apps-ui-kit';
import { Warning } from '@iota/ui-icons';

export function CompletedTransactions() {
    const activeAddress = useActiveAddress();
    const { data: txns, isPending, error } = useQueryTransactionsByAddress(activeAddress);
    if (error) {
        return (
            <div className="mb-2 flex h-full w-full items-center justify-center p-2">
                <InfoBox
                    type={InfoBoxType.Warning}
                    title="Something went wrong"
                    supportingText={error?.message ?? 'An error occurred'}
                    icon={<Warning />}
                    style={InfoBoxStyle.Default}
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
