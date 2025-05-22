// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCurrentAccount } from '@iota/dapp-kit';
import { TransactionTile } from '@/components';
import { NoData, VirtualList, useQueryTransactionsByAddress } from '@iota/core';
import { getExtendedTransaction } from '@/lib/utils/transaction';
import { IotaTransactionBlockResponse } from '@iota/iota-sdk/client';

interface TransactionsListProps {
    heightClassName?: string;
    displayImage?: boolean;
}

export function TransactionsList({
    heightClassName,
    displayImage,
}: TransactionsListProps): JSX.Element {
    const currentAccount = useCurrentAccount();
    const { allTransactions, fetchNextPage, hasNextPage, isFetchingNextPage, error } =
        useQueryTransactionsByAddress(currentAccount?.address);

    if (error) {
        return <div>{error?.message}</div>;
    }

    const virtualItem = (rawTransaction: IotaTransactionBlockResponse): JSX.Element => {
        const transaction = getExtendedTransaction(rawTransaction, currentAccount?.address || '');
        return <TransactionTile transaction={transaction} />;
    };

    return (
        <div>
            {allTransactions?.length == 0 ? (
                <div className="py-2xl">
                    <NoData
                        message="You can view your IOTA network transactions here once they are available."
                        displayImage={displayImage}
                    />
                </div>
            ) : (
                <VirtualList
                    items={allTransactions || []}
                    getItemKey={(tx) => tx?.digest}
                    estimateSize={() => 60}
                    render={virtualItem}
                    fetchNextPage={fetchNextPage}
                    hasNextPage={hasNextPage}
                    isFetchingNextPage={isFetchingNextPage}
                    heightClassName={heightClassName}
                />
            )}
        </div>
    );
}
