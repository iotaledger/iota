// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCurrentAccount } from '@iota/dapp-kit';
import { TransactionTile } from '@/components';
import { VirtualList, useQueryTransactionsByAddress } from '@iota/core';
import { getExtendedTransaction } from '@/lib/utils/transaction';
import { IotaTransactionBlockResponse } from '@iota/iota-sdk/client';

interface TransactionsListProps {
    heightClassName?: string;
}

export function TransactionsList({ heightClassName }: TransactionsListProps): JSX.Element {
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
    );
}
