// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DropdownPosition, Select, SelectSize } from '@iota/apps-ui-kit';
import { useState } from 'react';
import { PlaceholderTable, TableCard } from '~/components/ui';
import { useCursorPagination } from '@iota/core';
import {
    DEFAULT_TRANSACTIONS_LIMIT,
    useGetTransactionBlocks,
} from '~/hooks/useGetTransactionBlocks';
import { PAGE_SIZES_RANGE_20_60 } from '~/lib/constants';
import { generateTransactionsTableColumns } from '~/lib/ui';

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
                    <TableCard
                        data={data.data}
                        columns={tableColumns}
                        paginationOptions={pagination}
                        pageSizeSelector={
                            <Select
                                dropdownPosition={DropdownPosition.Top}
                                value={limit.toString()}
                                options={PAGE_SIZES_RANGE_20_60.map((size) => ({
                                    label: `${size} / page`,
                                    id: size.toString(),
                                }))}
                                onValueChange={(value) => {
                                    setLimit(Number(value));
                                    pagination.onFirst();
                                }}
                                size={SelectSize.Small}
                            />
                        }
                    />
                </div>
            )}
        </div>
    );
}
