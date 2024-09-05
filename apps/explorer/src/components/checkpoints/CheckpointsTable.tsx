// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientQuery } from '@iota/dapp-kit';
import { Select } from '@iota/apps-ui-kit';
import { ArrowRight } from '@iota/ui-icons';
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
        <div className="flex flex-col gap-md text-left xl:pr-10">
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
                <div>
                    <TableCard data={cardData.data} columns={cardData.columns} />
                </div>
            )}

            <div className="flex justify-between">
                {!disablePagination ? (
                    <Pagination
                        {...pagination}
                        hasNext={
                            maxCursor
                                ? Number(data && data.nextCursor) > Number(maxCursor)
                                : pagination.hasNext
                        }
                    />
                ) : (
                    <Link
                        to="/recent?tab=checkpoints"
                        after={<ArrowRight className="h-3 w-3 -rotate-45" />}
                    >
                        View all
                    </Link>
                )}

                <div className="flex items-center gap-md">
                    <div className="whitespace-nowrap text-label-md text-neutral-40">
                        {count ? numberSuffix(Number(count)) : '-'}
                        {` Total`}
                    </div>
                    {!disablePagination && (
                        <div className="[&_button]:px-2 [&_button]:py-1">
                            <Select
                                options={[
                                    {
                                        id: '20',
                                        label: '20 Per Page',
                                    },
                                    {
                                        id: '40',
                                        label: '40 Per Page',
                                    },
                                    {
                                        id: '60',
                                        label: '60 Per Page',
                                    },
                                ]}
                                value={String(limit)}
                                onValueChange={(e) => {
                                    setLimit(Number(e));
                                    pagination.onFirst();
                                }}
                            />
                        </div>
                    )}
                </div>
            </div>
        </div>
    );
}
