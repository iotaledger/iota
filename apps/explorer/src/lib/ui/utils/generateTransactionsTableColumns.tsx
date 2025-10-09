// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getTotalGasUsed } from '@iota/core';
import type { IotaTransactionBlockKind, IotaTransactionBlockResponse } from '@iota/iota-sdk/client';

import { TableCellBase, TableCellText } from '@iota/apps-ui-kit';
import type { ColumnDef } from '@tanstack/react-table';
import { AddressLink, TransactionLink } from '../../../components/ui';
import {
    CoinFormat,
    formatBalance,
    formatDigest,
    IOTA_TYPE_ARG,
    NANOS_PER_IOTA,
} from '@iota/iota-sdk/utils';
import { getElapsedTime } from '~/pages/epochs/utils';

/**
 * Generate table columns renderers for the transactions data.
 */
export function generateTransactionsTableColumns(
    address?: string,
): ColumnDef<IotaTransactionBlockResponse>[] {
    const columns: ColumnDef<IotaTransactionBlockResponse>[] = [
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
                            copyText={address}
                            className="[&>div]:max-w-[200px] [&>div]:truncate"
                            display="block"
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
    ];

    if (address) {
        columns.push({
            header: 'Balance Change',
            accessorKey: 'balanceChanges',
            cell: ({ getValue }) => {
                const balanceChanges = getValue<IotaTransactionBlockResponse['balanceChanges']>();
                if (!balanceChanges) {
                    return (
                        <TableCellBase>
                            <TableCellText>--</TableCellText>
                        </TableCellBase>
                    );
                }
                const balanceChange = balanceChanges.find(
                    (change) =>
                        change.owner &&
                        typeof change.owner === 'object' &&
                        'AddressOwner' in change.owner &&
                        change.owner.AddressOwner === address &&
                        change.coinType === IOTA_TYPE_ARG,
                );
                if (!balanceChange) {
                    return (
                        <TableCellBase>
                            <TableCellText>--</TableCellText>
                        </TableCellBase>
                    );
                }
                const amount = balanceChange.amount;
                const formatted = formatBalance(
                    Math.abs(Number(amount)) / Number(NANOS_PER_IOTA),
                    0,
                    CoinFormat.Rounded,
                );
                const sign = Number(amount) >= 0 ? '+' : '-';
                return (
                    <TableCellBase>
                        <TableCellText supportingLabel="IOTA">{sign + formatted}</TableCellText>
                    </TableCellBase>
                );
            },
        });
    }

    columns.push(
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
                          CoinFormat.Rounded,
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
    );

    return columns;
}
