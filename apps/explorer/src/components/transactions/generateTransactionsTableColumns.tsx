// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getTotalGasUsed } from '@iota/core';
import { IotaTransactionBlockKind, type IotaTransactionBlockResponse } from '@iota/iota-sdk/client';

import { TableBaseCell, TableCellText } from '@iota/apps-ui-kit';
import { ColumnDef } from '@tanstack/react-table';
import { AddressLink, ObjectLink } from '../ui';

/**
 * Generate table columns renderers for the transactions data.
 */
export function generateTransactionsTableColumns(): ColumnDef<IotaTransactionBlockResponse>[] {
    return [
        {
            header: 'Digest',
            accessorKey: 'digest',
            cell: ({ getValue }) => {
                const digest = getValue<string>();
                return (
                    <TableBaseCell>
                        <ObjectLink
                            objectId={digest}
                            label={<TableCellText>{digest}</TableCellText>}
                        />
                    </TableBaseCell>
                );
            },
        },
        {
            header: 'Sender',
            accessorKey: 'transaction.data.sender',
            cell: ({ getValue }) => {
                const address = getValue<string>();
                return (
                    <TableBaseCell>
                        <AddressLink
                            address={address}
                            label={<TableCellText>{address}</TableCellText>}
                        />
                    </TableBaseCell>
                );
            },
        },
        {
            header: 'Txns',
            accessorKey: 'transaction.data.transaction',
            cell: ({ getValue }) => {
                const transaction = getValue<IotaTransactionBlockKind>();
                const txns =
                    transaction.kind === 'ProgrammableTransaction'
                        ? transaction.transactions.length.toString()
                        : '--';
                return (
                    <TableBaseCell>
                        <TableCellText>{txns}</TableCellText>
                    </TableBaseCell>
                );
            },
        },
        {
            header: 'Gas',
            accessorKey: 'effects',
            cell: ({ getValue }) => {
                const effects = getValue<IotaTransactionBlockResponse['effects']>();
                return (
                    <TableBaseCell>
                        <TableCellText>
                            {effects ? getTotalGasUsed(effects)?.toString() : '0'}
                        </TableCellText>
                    </TableBaseCell>
                );
            },
        },
        {
            header: 'Time',
            accessorKey: 'timestampMs',
            cell: ({ getValue }) => {
                const timestampMs = getValue();
                return (
                    <TableBaseCell>
                        <TableCellText>{timestampMs?.toString() ?? '--'}</TableCellText>
                    </TableBaseCell>
                );
            },
        },
    ];
}
