// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CoinFormat, formatBalance, getTotalGasUsed } from '@iota/core';
import type { IotaTransactionBlockKind, IotaTransactionBlockResponse } from '@iota/iota-sdk/client';

import { TableCellBase, TableCellText } from '@iota/apps-ui-kit';
import type { ColumnDef } from '@tanstack/react-table';
import { AddressLink, TransactionLink } from '../../../components/ui';
import { formatAddress, formatDigest, NANOS_PER_IOTA } from '@iota/iota-sdk/utils';
import { getElapsedTime } from '~/pages/epochs/utils';
import { onCopySuccess } from '~/lib/utils';

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
                            label={<TableCellText>{formatDigest(digest)}</TableCellText>}
                            copyText={digest}
                            onCopySuccess={onCopySuccess}
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
                            copyText={address}
                            onCopySuccess={onCopySuccess}
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
                    ? formatBalance(
                          Number(totalGasUsed) / Number(NANOS_PER_IOTA),
                          0,
                          CoinFormat.ROUNDED,
                      )
                    : '--';
                return (
                    <TableCellBase>
                        <TableCellText supportingLabel={totalGasUsed ? 'IOTA' : undefined}>
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
                const elapsedTime = timestampMs
                    ? getElapsedTime(Number(timestampMs), Date.now())
                    : '--';
                return (
                    <TableCellBase>
                        <TableCellText>{elapsedTime}</TableCellText>
                    </TableCellBase>
                );
            },
        },
    ];
}
