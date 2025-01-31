// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { InfoBox, InfoBoxStyle, InfoBoxType, Select, SelectSize } from '@iota/apps-ui-kit';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { Warning } from '@iota/apps-ui-icons';
import { useMemo, useState } from 'react';
import { PlaceholderTable, TableCard } from '~/components/ui';
import { DEFAULT_CHECKPOINTS_LIMIT, useGetCheckpoints } from '~/hooks/useGetCheckpoints';
import { PAGE_SIZES_RANGE_20_60 } from '~/lib/constants';
import { generateCheckpointsTableColumns } from '~/lib/ui';
import { numberSuffix } from '~/lib/utils';
import { useCursorPagination } from '@iota/core';

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

    const tableColumns = generateCheckpointsTableColumns();

    return (
        <div className="flex flex-col gap-md text-left xl:pr-10">
            {isError && (
                <InfoBox
                    title="Error"
                    supportingText="Failed to load Checkpoints"
                    icon={<Warning />}
                    type={InfoBoxType.Error}
                    style={InfoBoxStyle.Default}
                />
            )}
            {isPending || isFetching || !data?.data ? (
                <PlaceholderTable
                    rowCount={Number(limit)}
                    rowHeight="16px"
                    colHeadings={['Digest', 'Sequence Number', 'Time', 'Transaction Count']}
                />
            ) : (
                <TableCard
                    data={data.data}
                    columns={tableColumns}
                    totalLabel={count ? `${numberSuffix(Number(count))} Total` : '-'}
                    viewAll={!disablePagination ? '/recent?tab=checkpoints' : undefined}
                    paginationOptions={
                        !disablePagination
                            ? {
                                  ...pagination,
                                  hasNext: maxCursor
                                      ? Number(data && data.nextCursor) > Number(maxCursor)
                                      : pagination.hasNext,
                              }
                            : undefined
                    }
                    pageSizeSelector={
                        !disablePagination && (
                            <Select
                                value={limit.toString()}
                                options={PAGE_SIZES_RANGE_20_60.map((size) => ({
                                    label: `${size} / page`,
                                    id: size.toString(),
                                }))}
                                size={SelectSize.Small}
                                onValueChange={(e) => {
                                    setLimit(Number(e));
                                    pagination.onFirst();
                                }}
                            />
                        )
                    }
                />
            )}
        </div>
    );
}
