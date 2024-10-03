// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getTotalGasUsed } from '@iota/core';
import type { IotaTransactionBlockKind, IotaTransactionBlockResponse } from '@iota/iota-sdk/client';

import { TableCellBase, TableCellText } from '@iota/apps-ui-kit';
import type { ColumnDef } from '@tanstack/react-table';
import { AddressLink, TransactionLink } from '../../../components/ui';
import { formatAddress } from '@iota/iota-sdk/utils';
import { getElapsedTime } from '~/pages/epochs/utils';

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
                    <TableCellBase>
                        <TransactionLink
                            digest={digest}
                            label={<TableCellText>{formatAddress(digest)}</TableCellText>}
                        />
                    </TableCellBase>
                );
            },
        },
        {
            header: 'Sender',
            accessorKey: 'transaction.data.sender',
            cell: ({ getValue }) => {
                const address = getValue<string>();
                return (
                    <TableCellBase>
                        <AddressLink
                            address={address}
                            label={<TableCellText>{formatAddress(address)}</TableCellText>}
                        />
                    </TableCellBase>
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
                    <TableCellBase>
                        <TableCellText>{txns}</TableCellText>
                    </TableCellBase>
                );
            },
        },
        {
            header: 'Gas',
            accessorKey: 'effects',
            cell: ({ getValue }) => {
                const effects = getValue<IotaTransactionBlockResponse['effects']>();
                const totalGasUsed = effects ? getTotalGasUsed(effects)?.toString() : undefined;
                const totalGasUsedFormatted = totalGasUsed
                    ? totalGasUsed
                    : '--';
                return (
                    <TableCellBase>
                        <TableCellText supportingLabel={totalGasUsed ? 'nano' : undefined}>
                            {totalGasUsedFormatted}
                        </TableCellText>
                    </TableCellBase>
                );
            },
        },
        {
            header: 'Time',
            accessorKey: 'timestampMs',
            cell: ({ getValue }) => {
                const timestampMs = getValue();
                return (
                    <TableCellBase>
                        <TableCellText>
                            {getElapsedTime(Number(timestampMs), Date.now()) || '--'}
                        </TableCellText>
                    </TableCellBase>
                );
            },
        },
    ];
}
