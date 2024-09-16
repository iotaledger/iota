// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { EpochMetrics, type EpochMetricsPage } from '@iota/iota-sdk/client';
import { ColumnDef } from '@tanstack/react-table';
import { TableCellText } from '@iota/apps-ui-kit';

/**
 * Generate table columns renderers for the epochs data.
 */
export function generateEpochTableColumns(): ColumnDef<EpochMetrics>[] {
    return [
        {
            header: 'Epoch',
            accessorKey: 'epoch',
            cell: ({ getValue }) => {
                const epoch = getValue<EpochMetrics['epoch']>();
                return (
                    <TableCellText>
                        {epoch}
                    </TableCellText>
                )
            }
        },
        {
            header: 'Transaction Blocks',
            accessorKey: 'transactions',
            cell: ({ getValue }) => {
                const epoch = getValue<EpochMetrics['epoch']>();
                return (
                    <TableCellText>
                        {epoch}
                    </TableCellText>
                )
            }
        },
        {
            header: 'Stake Rewards',
            accessorKey: 'stakeRewards',
            cell: ({ getValue }) => {
                const epoch = getValue<EpochMetrics['epoch']>();
                return (
                    <TableCellText>
                        {epoch}
                    </TableCellText>
                )
            }
        },
        {
            header: 'Checkpoint Set',
            accessorKey: 'checkpointSet',
            cell: ({ getValue }) => {
                const epoch = getValue<EpochMetrics['epoch']>();
                return (
                    <TableCellText>
                        {epoch}
                    </TableCellText>
                )
            }
        },
        {
            header: 'Storage Net Inflow',
            accessorKey: 'storageNetInflow',
            cell: ({ getValue }) => {
                const epoch = getValue<EpochMetrics['epoch']>();
                return (
                    <TableCellText>
                        {epoch}
                    </TableCellText>
                )
            }
        },
        {
            header: 'Epoch End',
            accessorKey: 'time',
            cell: ({ getValue }) => {
                const epoch = getValue<EpochMetrics['epoch']>();
                return (
                    <TableCellText>
                        {epoch}
                    </TableCellText>
                )
            }
        },
    ];
}
