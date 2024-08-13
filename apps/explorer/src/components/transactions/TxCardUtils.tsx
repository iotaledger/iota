// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getTotalGasUsed } from '@iota/core';
import { X12, Dot12 } from '@iota/icons';
import { type IotaClient, type IotaTransactionBlockResponse } from '@iota/iota-sdk/client';

import { IotaAmount } from '../table/IotaAmount';
import { TxTimeType } from '../tx-time/TxTimeType';
import { HighlightedTableCol } from '~/components';
import { AddressLink, TransactionLink } from '~/components/ui';
import { type ReactNode } from 'react';

interface TransactionData {
    date: ReactNode;
    digest: ReactNode;
    txns: ReactNode;
    gas: ReactNode;
    sender: ReactNode;
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
            const status = transaction.effects?.status.status;
            const sender = transaction.transaction?.data.sender;

            return {
                date: (
                    <HighlightedTableCol>
                        <TxTimeType timestamp={Number(transaction.timestampMs || 0)} />
                    </HighlightedTableCol>
                ),
                digest: (
                    <HighlightedTableCol first>
                        <TransactionLink
                            digest={transaction.digest}
                            before={
                                status === 'success' ? (
                                    <Dot12 className="text-success" />
                                ) : (
                                    <X12 className="text-issue-dark" />
                                )
                            }
                        />
                    </HighlightedTableCol>
                ),
                txns: (
                    <div>
                        {transaction.transaction?.data.transaction.kind ===
                        'ProgrammableTransaction'
                            ? transaction.transaction.data.transaction.transactions.length
                            : '--'}
                    </div>
                ),
                gas: (
                    <IotaAmount
                        amount={transaction.effects ? getTotalGasUsed(transaction.effects) : 0}
                    />
                ),
                sender: (
                    <HighlightedTableCol>
                        {sender ? <AddressLink address={sender} /> : '-'}
                    </HighlightedTableCol>
                ),
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
