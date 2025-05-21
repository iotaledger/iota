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
        <div>
            {allTransactions?.length == 0 ? (
                <div className="flex flex-col gap-md">
                    <div className="flex flex-col flex-nowrap items-center justify-center px-sm text-center">
                        <span className="text-body-sm text-neutral-40 dark:text-neutral-60">
                            {
                                'You can view your IOTA network transactions here once they are available.'
                            }
                        </span>
                    </div>
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
