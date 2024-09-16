// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getTotalGasUsed } from '@iota/core';
import { type IotaTransactionBlockResponse } from '@iota/iota-sdk/client';

import { TableCellText } from '@iota/apps-ui-kit';
import { ColumnDef } from '@tanstack/react-table';
import { AddressLink, ObjectLink } from '../ui';

/**
 * Generate table columns renderers for the transactions data.
 */
export function generateTransactionsTableColumns(): ColumnDef<IotaTransactionBlockResponse>[] {
    return [
        {
            header: 'Date',
            accessorKey: "timestampMs",
            cell: ({ getValue }) => {
                const timestampMs = getValue();
                return (
                    <TableCellText>
                        {timestampMs?.toString()}
                    </TableCellText>
                )
            }
        },
        {
            header: 'Digest',
            accessorKey: 'digest',
            cell: ({ getValue }) => {
                const digest = getValue<string>();
                return (
                    <ObjectLink objectId={digest} label={
                        <TableCellText>
                            {digest}
                        </TableCellText>
                    }/>
                )
            }
        },
        {
            header: 'Sender',
            accessorKey: 'transaction',
            cell: ({ getValue }) => {
                const transaction = getValue<IotaTransactionBlockResponse["transaction"]>();
                const address = transaction?.data.sender.toString() || '';
                return (
                    <AddressLink address={address} label={
                        <TableCellText>
                            {address}
                        </TableCellText>
                    }/>
                )
            }
        },
        {
            header: 'Txns',
            accessorKey: 'transaction',
            cell: ({ getValue }) => {
                const transaction = getValue<IotaTransactionBlockResponse["transaction"]>();
                return (
                    <TableCellText>
                        {transaction?.data.transaction.kind === 'ProgrammableTransaction'
                            ? transaction.data.transaction.transactions.length.toString()
                            : '--'}
                    </TableCellText>
                )
            }
        },
        {
            header: 'Gas',
            accessorKey: 'effects',
            cell: ({ getValue }) => {
                const effects = getValue<IotaTransactionBlockResponse["effects"]>();
                return (
                    <TableCellText>
                        {effects
                        ? getTotalGasUsed(effects)?.toString()
                        : '0'}
                    </TableCellText>
                )
            }
        },
    ];
}

