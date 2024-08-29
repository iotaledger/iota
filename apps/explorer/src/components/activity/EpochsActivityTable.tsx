// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient, useIotaClientInfiniteQuery } from '@iota/dapp-kit';
import { ArrowRight12 } from '@iota/icons';
import { Text } from '@iota/ui';
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';

import {
    Link,
    Pagination,
    PlaceholderTable,
    TableCard,
    useCursorPagination,
} from '~/components/ui';
import { generateTableDataFromEpochsData } from '~/lib/ui';
import { numberSuffix } from '~/lib/utils';

const DEFAULT_EPOCHS_LIMIT = 20;

interface EpochsActivityTableProps {
    disablePagination?: boolean;
    refetchInterval?: number;
    initialLimit?: number;
}

export function EpochsActivityTable({
    disablePagination,
    initialLimit = DEFAULT_EPOCHS_LIMIT,
}: EpochsActivityTableProps): JSX.Element {
    const [limit, setLimit] = useState(initialLimit);
    const client = useIotaClient();

    const { data: count } = useQuery({
        queryKey: ['epochs', 'current'],
        queryFn: async () => client.getCurrentEpoch(),
        select: (epoch) => Number(epoch.epoch) + 1,
    });

    const epochMetricsQuery = useIotaClientInfiniteQuery('getEpochMetrics', {
        limit,
        descendingOrder: true,
    });
    const { data, isFetching, pagination, isPending, isError } =
        useCursorPagination(epochMetricsQuery);

    const cardData = data ? generateTableDataFromEpochsData(data) : undefined;

    return (
        <div className="flex flex-col space-y-3 text-left xl:pr-10">
            {isError && (
                <div className="pt-2 font-sans font-semibold text-issue-dark">
                    Failed to load Epochs
                </div>
            )}
            {isPending || isFetching || !cardData ? (
                <PlaceholderTable
                    rowCount={limit}
                    rowHeight="16px"
                    colHeadings={[
                        'Epoch',
                        'Transaction Blocks',
                        'Stake Rewards',
                        'Checkpoint Set',
                        'Storage Net Inflow',
                        'Epoch End',
                    ]}
                    colWidths={['100px', '120px', '40px', '204px', '90px', '38px']}
                />
            ) : (
                <TableCard data={cardData.data} columns={cardData.columns} paginationOptions={ !disablePagination ? {
                    onFirstPageClick: pagination.hasPrev ? pagination.onFirst : undefined,
                    onNextPageClick: pagination.hasNext ? pagination.onNext : undefined,
                    onPreviousPageClick: pagination.hasPrev ? pagination.onPrev : undefined,
                } : undefined}  />
            )}

            <div className="flex justify-between">
                {disablePagination ? (
                    <Link
                    to="/recent?tab=epochs"
                    after={<ArrowRight12 className="h-3 w-3 -rotate-45" />}
                >
                    View all
                </Link>
                ) : undefined}

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
    );
}
