// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type TableCellProps, TableCellType } from '@iota/apps-ui-kit';
import { type CheckpointPage } from '@iota/iota-sdk/client';

interface CheckpointData {
    digest: TableCellProps;
    time: TableCellProps;
    sequenceNumber: TableCellProps;
    transactionBlockCount: TableCellProps;
}

interface TableColumn {
    header: string;
    accessorKey: keyof CheckpointData;
}

interface CheckpointTableData {
    data: CheckpointData[];
    columns: TableColumn[];
}

// Generate table data from the checkpoints data
export function generateTableDataFromCheckpointsData(results: CheckpointPage): CheckpointTableData {
    return {
        data:
            results.data.map((checkpoint) => ({
                digest: { type: TableCellType.Text, label: checkpoint.digest },
                time: { type: TableCellType.Text, label: checkpoint.timestampMs },
                sequenceNumber: { type: TableCellType.Text, label: checkpoint.sequenceNumber },
                transactionBlockCount: {
                    type: TableCellType.Text,
                    label: checkpoint.transactions.length.toString(),
                },
            })) ?? [],
        columns: [
            {
                header: 'Digest',
                accessorKey: 'digest',
            },
            {
                header: 'Sequence Number',
                accessorKey: 'sequenceNumber',
            },
            {
                header: 'Time',
                accessorKey: 'time',
            },
            {
                header: 'Transaction Block Count',
                accessorKey: 'transactionBlockCount',
            },
        ],
    };
}
