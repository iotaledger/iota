// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';
import { useEffect, useRef, useState } from 'react';
import { PlaceholderTable, TableCard } from '~/components/ui';
import { useCursorPagination } from '@iota/core';
import {
    DEFAULT_TRANSACTIONS_LIMIT,
    useGetTransactionBlocks,
} from '~/hooks/useGetTransactionBlocks';
import { numberSuffix } from '~/lib/utils';
import { InfoBox, InfoBoxStyle, InfoBoxType, Select, SelectSize } from '@iota/apps-ui-kit';
import { generateTransactionsTableColumns } from '~/lib/ui';
import { Warning } from '@iota/apps-ui-icons';
import { PAGE_SIZES_RANGE_20_60 } from '~/lib/constants';
import { type IotaTransactionKind } from '@iota/iota-sdk/client';

interface TransactionsActivityTableProps {
    disablePagination?: boolean;
    refetchInterval?: number;
    initialLimit?: number;
    transactionKindFilter?: IotaTransactionKind;
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
    const tableColumns = generateTransactionsTableColumns();

    useEffect(() => {
        goToFirstPageRef.current();
    }, [transactionKindFilter]);
    return (
        <div data-testid="tx">
            {isError && (
                <InfoBox
                    title="Error"
                    supportingText="Failed to load Transactions"
                    icon={<Warning />}
                    type={InfoBoxType.Error}
                    style={InfoBoxStyle.Default}
                />
            )}
            <div className="flex flex-col space-y-3 text-left">
                {isPending || isFetching || !data?.data ? (
                    <PlaceholderTable
                        rowCount={limit}
                        rowHeight="16px"
                        colHeadings={['Digest', 'Sender', 'Txns', 'Gas', 'Time']}
                    />
                ) : (
                    <TableCard
                        data={data.data}
                        columns={tableColumns}
                        totalLabel={count ? `${numberSuffix(Number(count))} Total` : '-'}
                        viewAll="/recent"
                        paginationOptions={!disablePagination ? pagination : undefined}
                        pageSizeSelector={
                            !disablePagination && (
                                <Select
                                    value={limit.toString()}
                                    options={PAGE_SIZES_RANGE_20_60.map((size) => ({
                                        label: `${size} / page`,
                                        id: size.toString(),
                                    }))}
                                    onValueChange={(e) => {
                                        setLimit(Number(e));
                                        pagination.onFirst();
                                    }}
                                    size={SelectSize.Small}
                                />
                            )
                        }
                    />
                )}
            </div>
        </div>
    );
}
