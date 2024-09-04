// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Select } from '@iota/apps-ui-kit';
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
                <TableCard
                    data={cardData.data}
                    columns={cardData.columns}
                    totalLabel={count ? `${numberSuffix(Number(count))} Total` : '-'}
                    viewAll={!disablePagination ? '/recent?tab=epochs' : undefined}
                    paginationOptions={
                        !disablePagination
                            ? {
                                  onFirstPageClick: pagination.hasPrev
                                      ? pagination.onFirst
                                      : undefined,
                                  onNextPageClick: pagination.hasNext
                                      ? pagination.onNext
                                      : undefined,
                                  onPreviousPageClick: pagination.hasPrev
                                      ? pagination.onPrev
                                      : undefined,
                              }
                            : undefined
                    }
                />
            )}

            <div className="flex justify-between">
                <div className="flex items-center space-x-3">
                    {!disablePagination && (
                        <Select
                            value={limit.toString()}
                            options={[
                                { id: '20', label: '20' },
                                { id: '40', label: '40' },
                                { id: '60', label: '60' },
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
    );
}
