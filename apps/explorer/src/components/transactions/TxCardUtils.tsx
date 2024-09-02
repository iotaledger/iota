// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getTotalGasUsed } from '@iota/core';
import { type IotaClient, type IotaTransactionBlockResponse } from '@iota/iota-sdk/client';

import { type TableCellProps, TableCellType } from '@iota/apps-ui-kit';

interface TransactionData {
    date: TableCellProps;
    digest: TableCellProps;
    txns: TableCellProps;
    gas: TableCellProps;
    sender: TableCellProps;
}

interface TableColumn {
    header: string;
    accessorKey: keyof TransactionData;
}

// Generate table data from the transaction data

export function genTableDataFromTxData(results: IotaTransactionBlockResponse[]): {
    data: TransactionData[];
    columns: TableColumn[];
} {
    return {
        data: results.map((transaction) => {
            const sender = transaction.transaction?.data.sender;

            return {
                date: { type: TableCellType.Text, label: transaction.timestampMs?.toString() },
                digest: {
                    type: TableCellType.TextToCopy,
                    label: transaction.digest,
                    textToCopy: transaction.digest,
                },
                txns: {
                    type: TableCellType.Text,
                    label:
                        transaction.transaction?.data.transaction.kind === 'ProgrammableTransaction'
                            ? transaction.transaction.data.transaction.transactions.length.toString()
                            : '--',
                },
                gas: {
                    type: TableCellType.Text,
                    label: transaction.effects
                        ? getTotalGasUsed(transaction.effects)?.toString()
                        : '0',
                },
                sender: { type: TableCellType.TextToCopy, label: sender, textToCopy: sender },
            };
        }),
        columns: [
            {
                header: 'Digest',
                accessorKey: 'digest',
            },
            {
                header: 'Sender',
                accessorKey: 'sender',
            },
            {
                header: 'Txns',
                accessorKey: 'txns',
            },
            {
                header: 'Gas',
                accessorKey: 'gas',
            },
            {
                header: 'Time',
                accessorKey: 'date',
            },
        ],
    };
}

const dedupe = (arr: string[]) => Array.from(new Set(arr));

export function getDataOnTxDigests(
    client: IotaClient,
    transactions: string[],
): Promise<IotaTransactionBlockResponse[]> {
    return client
        .multiGetTransactionBlocks({
            digests: dedupe(transactions),
            options: {
                showInput: true,
                showEffects: true,
                showEvents: true,
            },
        })
        .then((transactions) =>
            // Remove failed transactions
            transactions.filter((item) => item),
        );
}
