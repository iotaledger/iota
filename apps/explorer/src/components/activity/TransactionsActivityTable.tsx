// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';
import { useEffect, useRef, useState } from 'react';

import { PlaceholderTable, TableCard, useCursorPagination } from '~/components/ui';
import {
    DEFAULT_TRANSACTIONS_LIMIT,
    useGetTransactionBlocks,
} from '~/hooks/useGetTransactionBlocks';
import { numberSuffix } from '~/lib/utils';
import { genTableDataFromTxData } from '../transactions/TxCardUtils';
import { Select } from '@iota/apps-ui-kit';

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
                    />
                ) : (
                    <TableCard
                        data={cardData.data}
                        columns={cardData.columns}
                        totalLabel={count ? `${numberSuffix(Number(count))} Total` : '-'}
                        viewAll={true ? '/recent' : undefined}
                        paginationOptions={!disablePagination ? pagination : undefined}
                    />
                )}

                <div className="flex justify-between">
                    <div className="flex items-center space-x-3">
                        {!disablePagination && (
                            <Select
                                value={limit.toString()}
                                options={[
                                    { id: '20', label: '20 Per Page' },
                                    { id: '40', label: '40 Per Page' },
                                    { id: '60', label: '60 Per Page' },
                                ]}
                                onValueChange={(e) => {
                                    setLimit(Number(e));
                                    pagination.onFirst();
                                }}
                            />
                        )}
                    </div>
                </div>
            </div>
        </div>
    );
}
