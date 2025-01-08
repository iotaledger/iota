// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TableCellBase, TableCellText } from '@iota/apps-ui-kit';
import type { Checkpoint } from '@iota/iota-sdk/client';
import type { ColumnDef } from '@tanstack/react-table';
import { CheckpointSequenceLink, CheckpointLink } from '~/components';
import { getElapsedTime } from '~/pages/epochs/utils';

/**
 * Generate table columns renderers for the checkpoints data.
 */
export function generateCheckpointsTableColumns(): ColumnDef<Checkpoint>[] {
    return [
        {
            header: 'Digest',
            accessorKey: 'digest',
            cell: ({ getValue }) => {
                const digest = getValue<Checkpoint['digest']>();
                return (
                    <TableCellBase>
                        <CheckpointLink
                            digest={digest}
                            label={<TableCellText>{digest}</TableCellText>}
                        />
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
        {
            header: 'Time',
            accessorKey: 'timestampMs',
            cell: ({ getValue }) => {
                const timestampMs = getValue<Checkpoint['timestampMs']>();
                return (
                    <TableCellBase>
                        <TableCellText>
                            {timestampMs ? getElapsedTime(Number(timestampMs), Date.now()) : '--'}
                        </TableCellText>
                    </TableCellBase>
                );
            },
        },
    ];
}
