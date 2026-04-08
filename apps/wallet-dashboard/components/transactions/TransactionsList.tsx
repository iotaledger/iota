// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCurrentAccount } from '@iota/dapp-kit';
import { TransactionTile } from '@/components';
import { NoData, VirtualList, useQueryTransactionsByAddress } from '@iota/core';
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

    const virtualItem = (transaction: IotaTransactionBlockResponse): JSX.Element => {
        return <TransactionTile transaction={transaction} />;
    };

    if (!allTransactions || allTransactions.length === 0) {
        return (
            <NoData
                message="You can view your IOTA network transactions here once they are available."
                displayImage={displayImage}
            />
        );
    }

    return (
        <VirtualList
            items={allTransactions}
            getItemKey={(tx) => tx?.digest}
            estimateSize={() => 60}
            render={virtualItem}
            fetchNextPage={fetchNextPage}
            hasNextPage={hasNextPage}
            isFetchingNextPage={isFetchingNextPage}
            heightClassName={heightClassName}
        />
    );
}
