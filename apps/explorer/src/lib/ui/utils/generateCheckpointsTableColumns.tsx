// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TableCellBase, TableCellText } from '@iota/apps-ui-kit';
import type { Checkpoint } from '@iota/iota-sdk/client';
import type { ColumnDef } from '@tanstack/react-table';
import { CheckpointSequenceLink } from '~/components';

/**
 * Generate table columns renderers for the checkpoints data.
 */
export function generateCheckpointsTableColumns(): ColumnDef<Checkpoint>[] {
    return [
        {
            header: 'Digest',
            accessorKey: 'digest',
            cell: ({ getValue }) => {
                const epoch = getValue<Checkpoint['digest']>();
                return (
                    <TableCellBase>
                        <TableCellText>{epoch}</TableCellText>
                    </TableCellBase>
                );
            },
        },
        {
            header: 'Sequence Number',
            accessorKey: 'sequenceNumber',
            cell: ({ getValue }) => {
                const sequenceNumber = getValue<Checkpoint['sequenceNumber']>();
                return (
                    <TableCellBase>
                        <TableCellText>
                            <CheckpointSequenceLink sequence={sequenceNumber}>
                                {sequenceNumber}
                            </CheckpointSequenceLink>
                        </TableCellText>
                    </TableCellBase>
                );
            },
        },
        {
            header: 'Transactions',
            accessorKey: 'networkTotalTransactions',
            cell: ({ getValue }) => {
                const networkTotalTransactions = getValue<Checkpoint['networkTotalTransactions']>();
                return (
                    <TableCellBase>
                        <TableCellText>{networkTotalTransactions}</TableCellText>
                    </TableCellBase>
                );
            },
        },
        {
            header: 'Time',
            accessorKey: 'timestampMs',
            cell: ({ getValue }) => {
                const timestampMs = getValue<Checkpoint['timestampMs']>();
                return (
                    <TableCellBase>
                        <TableCellText>{timestampMs}</TableCellText>
                    </TableCellBase>
                );
            },
        },
        {
            header: 'Transaction Block Count',
            accessorKey: 'transactions',
            cell: ({ getValue }) => {
                const transactions = getValue<Checkpoint['transactions']>();
                return (
                    <TableCellBase>
                        <TableCellText>{transactions.length}</TableCellText>
                    </TableCellBase>
                );
            },
        },
    ];
}
