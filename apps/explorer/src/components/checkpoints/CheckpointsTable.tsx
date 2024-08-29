// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientQuery } from '@iota/dapp-kit';
import { ArrowRight12 } from '@iota/icons';
import { Text } from '@iota/ui';
import { useMemo, useState } from 'react';

import {
    Link,
    Pagination,
    PlaceholderTable,
    TableCard,
    useCursorPagination,
} from '~/components/ui';
import { DEFAULT_CHECKPOINTS_LIMIT, useGetCheckpoints } from '~/hooks/useGetCheckpoints';
import { generateTableDataFromCheckpointsData } from '~/lib/ui';
import { numberSuffix } from '~/lib/utils';

interface CheckpointsTableProps {
    disablePagination?: boolean;
    refetchInterval?: number;
    initialLimit?: number;
    initialCursor?: string;
    maxCursor?: string;
}

export function CheckpointsTable({
    disablePagination,
    initialLimit = DEFAULT_CHECKPOINTS_LIMIT,
    initialCursor,
    maxCursor,
}: CheckpointsTableProps): JSX.Element {
    const [limit, setLimit] = useState(initialLimit);

    const countQuery = useIotaClientQuery('getLatestCheckpointSequenceNumber');

    const checkpoints = useGetCheckpoints(initialCursor, limit);

    const { data, isFetching, pagination, isPending, isError } = useCursorPagination(checkpoints);

    const count = useMemo(() => {
        if (maxCursor) {
            if (initialCursor) {
                return Number(initialCursor) - Number(maxCursor);
            } else if (!isError && checkpoints.data) {
                // Special case for ongoing epoch
                return Number(checkpoints.data.pages[0].data[0].sequenceNumber) - Number(maxCursor);
            }
        } else {
            return Number(countQuery.data ?? 0);
        }
    }, [countQuery.data, initialCursor, maxCursor, checkpoints, isError]);

    const cardData = data ? generateTableDataFromCheckpointsData(data) : undefined;

    return (
        <div className="flex flex-col space-y-3 text-left xl:pr-10">
            {isError && (
                <div className="pt-2 font-sans font-semibold text-issue-dark">
                    Failed to load Checkpoints
                </div>
            )}
            {isPending || isFetching || !cardData ? (
                <PlaceholderTable
                    rowCount={Number(limit)}
                    rowHeight="16px"
                    colHeadings={['Digest', 'Sequence Number', 'Time', 'Transaction Count']}
                    colWidths={['100px', '120px', '204px', '90px', '38px']}
                />
            ) : (
                <TableCard
                    data={cardData.data}
                    columns={cardData.columns}
                    paginationOptions={
                        !disablePagination
                            ? {
                                  onFirstPageClick: pagination.hasPrev
                                      ? pagination.onFirst
                                      : undefined,
                                  onNextPageClick: (
                                      maxCursor
                                          ? Number(data && data.nextCursor) > Number(maxCursor)
                                          : pagination.hasNext
                                  )
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
                {disablePagination ? (
                    <Link
                        to="/recent?tab=checkpoints"
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
