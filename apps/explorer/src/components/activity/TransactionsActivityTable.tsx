// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { ArrowRight12 } from '@iota/icons';
import { Text } from '@iota/ui';
import { useQuery } from '@tanstack/react-query';
import { useEffect, useRef, useState } from 'react';

import {
    Link,
    Pagination,
    PlaceholderTable,
    TableCard,
    useCursorPagination,
} from '~/components/ui';
import {
    DEFAULT_TRANSACTIONS_LIMIT,
    useGetTransactionBlocks,
} from '~/hooks/useGetTransactionBlocks';
import { numberSuffix } from '~/lib/utils';
import { genTableDataFromTxData } from '../transactions/TxCardUtils';

interface TransactionsActivityTableProps {
    disablePagination?: boolean;
    refetchInterval?: number;
    initialLimit?: number;
    transactionKindFilter?: 'ProgrammableTransaction';
}

export function TransactionsActivityTable({
    disablePagination,
    refetchInterval,
    initialLimit = DEFAULT_TRANSACTIONS_LIMIT,
    transactionKindFilter,
}: TransactionsActivityTableProps): JSX.Element {
    const [limit, setLimit] = useState(initialLimit);
    const client = useIotaClient();
    const { data: count } = useQuery({
        queryKey: ['transactions', 'count'],
        queryFn: () => client.getTotalTransactionBlocks(),
        gcTime: 24 * 60 * 60 * 1000,
        staleTime: Infinity,
        retry: false,
    });
    const transactions = useGetTransactionBlocks(
        transactionKindFilter ? { TransactionKind: transactionKindFilter } : undefined,
        limit,
        refetchInterval,
    );
    const { data, isFetching, pagination, isPending, isError } = useCursorPagination(transactions);
    const goToFirstPageRef = useRef(pagination.onFirst);
    goToFirstPageRef.current = pagination.onFirst;
    const cardData = data ? genTableDataFromTxData(data.data) : undefined;

    useEffect(() => {
        goToFirstPageRef.current();
    }, [transactionKindFilter]);
    return (
        <div data-testid="tx">
            {isError && (
                <div className="pt-2 font-sans font-semibold text-issue-dark">
                    Failed to load Transactions
                </div>
            )}
            <div className="flex flex-col space-y-3 text-left">
                {isPending || isFetching || !cardData ? (
                    <PlaceholderTable
                        rowCount={limit}
                        rowHeight="16px"
                        colHeadings={['Digest', 'Sender', 'Txns', 'Gas', 'Time']}
                        colWidths={['30%', '30%', '10%', '20%', '10%']}
                    />
                ) : (
                    <TableCard data={cardData.data} columns={cardData.columns} paginationOptions={ !disablePagination ? {
                        onFirstPageClick: pagination.hasPrev ?  pagination.onFirst : undefined,
                        onNextPageClick: pagination.hasNext ? pagination.onNext : undefined,
                        onPreviousPageClick: pagination.hasPrev ? pagination.onPrev : undefined,
                    } : undefined} />
                )}

                <div className="flex justify-between">
                    {disablePagination ? (
                        <Link to="/recent" after={<ArrowRight12 className="h-3 w-3 -rotate-45" />}>
                            View all
                        </Link>
                    ): undefined}
                    <div className="flex items-center space-x-3">
                        <Text variant="body/medium" color="steel-dark">
                            {count ? numberSuffix(Number(count)) : '-'}
                            {` Total`}
                        </Text>
                        {!disablePagination && (
                            <select
                                className="form-select rounded-md border border-gray-45 px-3 py-2 pr-8 text-bodySmall font-medium leading-[1.2] text-steel-dark shadow-button"
                                value={limit}
                                onChange={(e) => {
                                    setLimit(Number(e.target.value));
                                    pagination.onFirst();
                                }}
                            >
                                <option value={20}>20 Per Page</option>
                                <option value={40}>40 Per Page</option>
                                <option value={60}>60 Per Page</option>
                            </select>
                        )}
                    </div>
                </div>
            </div>
        </div>
    );
}
