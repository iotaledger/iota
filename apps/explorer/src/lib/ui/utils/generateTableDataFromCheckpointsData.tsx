// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type CheckpointPage } from '@iota/iota-sdk/client';
import { Text } from '@iota/ui';

import { type ReactNode } from 'react';
import { HighlightedTableCol, TxTimeType } from '~/components';
import { CheckpointLink, CheckpointSequenceLink } from '~/components/ui';

interface CheckpointData {
    digest: ReactNode;
    time: ReactNode;
    sequenceNumber: ReactNode;
    transactionBlockCount: ReactNode;
}

interface TableColumn {
    header: () => string;
    accessorKey: keyof CheckpointData;
}

interface CheckpointTableData {
    data: CheckpointData[];
    columns: TableColumn[];
}

// Generate table data from the checkpoints data
export function generateTableDataFromCheckpointsData(data: CheckpointPage): CheckpointTableData {
    return {
        data:
            data?.data.map((checkpoint) => ({
                digest: (
                    <HighlightedTableCol first>
                        <CheckpointLink digest={checkpoint.digest} />
                    </HighlightedTableCol>
                ),
                time: <TxTimeType timestamp={Number(checkpoint.timestampMs)} />,
                sequenceNumber: <CheckpointSequenceLink sequence={checkpoint.sequenceNumber} />,
                transactionBlockCount: (
                    <Text variant="bodySmall/medium" color="steel-darker">
                        {checkpoint.transactions.length}
                    </Text>
                ),
            })) ?? [],
        columns: [
            {
                header: () => 'Digest',
                accessorKey: 'digest',
            },
            {
                header: () => 'Sequence Number',
                accessorKey: 'sequenceNumber',
            },
            {
                header: () => 'Time',
                accessorKey: 'time',
            },
            {
                header: () => 'Transaction Block Count',
                accessorKey: 'transactionBlockCount',
            },
        ],
    };
}
