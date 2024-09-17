// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TableBaseCell, TableCellText } from '@iota/apps-ui-kit';
import { type Checkpoint, type CheckpointPage } from '@iota/iota-sdk/client';
import { ColumnDef } from '@tanstack/react-table';
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
                    <TableBaseCell isContentCentered>
                        <TableCellText>{epoch}</TableCellText>
                    </TableBaseCell>
                );
            },
        },
        {
            header: 'Sequence Number',
            accessorKey: 'sequenceNumber',
            cell: ({ getValue }) => {
                const sequenceNumber = getValue<Checkpoint['sequenceNumber']>();
                return (
                    <TableBaseCell isContentCentered>
                        <TableCellText>
                            <CheckpointSequenceLink sequence={sequenceNumber}>
                                {sequenceNumber}
                            </CheckpointSequenceLink>
                        </TableCellText>
                    </TableBaseCell>
                );
            },
        },
        {
            header: 'Transactions',
            accessorKey: 'networkTotalTransactions',
            cell: ({ getValue }) => {
                const networkTotalTransactions = getValue<Checkpoint['networkTotalTransactions']>();
                return (
                    <TableBaseCell isContentCentered>
                        <TableCellText>{networkTotalTransactions}</TableCellText>
                    </TableBaseCell>
                );
            },
        },
        {
            header: 'Time',
            accessorKey: 'timestampMs',
            cell: ({ getValue }) => {
                const timestampMs = getValue<Checkpoint['timestampMs']>();
                return (
                    <TableBaseCell isContentCentered>
                        <TableCellText>{timestampMs}</TableCellText>
                    </TableBaseCell>
                );
            },
        },
        {
            header: 'Transaction Block Count',
            accessorKey: 'transactions',
            cell: ({ getValue }) => {
                const transactions = getValue<Checkpoint['transactions']>();
                return (
                    <TableBaseCell isContentCentered>
                        <TableCellText>{transactions.length}</TableCellText>
                    </TableBaseCell>
                );
            },
        },
    ];
}
