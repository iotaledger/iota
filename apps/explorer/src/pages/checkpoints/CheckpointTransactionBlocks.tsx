// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';
import { generateTransactionsTableColumns } from '~/components/transactions/generateTransactionsTableColumns';

import { Pagination, PlaceholderTable, TableCard, useCursorPagination } from '~/components/ui';
import {
    DEFAULT_TRANSACTIONS_LIMIT,
    useGetTransactionBlocks,
} from '~/hooks/useGetTransactionBlocks';

export function CheckpointTransactionBlocks({ id }: { id: string }): JSX.Element {
    const [limit, setLimit] = useState(DEFAULT_TRANSACTIONS_LIMIT);
    const transactions = useGetTransactionBlocks(
        {
            Checkpoint: id,
        },
        limit,
    );

    const { data, isFetching, pagination, isPending } = useCursorPagination(transactions);

    const tableColumns = generateTransactionsTableColumns();

    return (
        <div className="flex flex-col space-y-5 text-left xl:pr-10">
            {isPending || isFetching || !data?.data ? (
                <PlaceholderTable
                    rowCount={20}
                    rowHeight="16px"
                    colHeadings={['Digest', 'Sender', 'Txns', 'Gas', 'Time']}
                />
            ) : (
                <div>
                    <TableCard data={data.data} columns={tableColumns} />
                </div>
            )}
            <div className="flex justify-between">
                <Pagination {...pagination} />
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
            </div>
        </div>
    );
}
