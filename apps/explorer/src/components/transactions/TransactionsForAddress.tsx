// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { InfoBox, InfoBoxStyle, InfoBoxType, LoadingIndicator } from '@iota/apps-ui-kit';
import { useIotaClient, useIotaIndexerClient } from '@iota/dapp-kit';
import { type IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { Warning } from '@iota/apps-ui-icons';
import { useQuery } from '@tanstack/react-query';
import { TableCard } from '~/components/ui';
import { generateTransactionsTableColumns } from '~/lib/ui';

interface TransactionsForAddressProps {
    address: string;
}

interface TransactionsForAddressTableProps {
    data: IotaTransactionBlockResponse[];
    isPending: boolean;
    isError: boolean;
    address: string;
}

export function TransactionsForAddressTable({
    data,
    isPending,
    isError,
    address,
}: TransactionsForAddressTableProps): JSX.Element {
    if (isPending) {
        return (
            <div>
                <LoadingIndicator />
            </div>
        );
    }

    if (isError) {
        return (
            <InfoBox
                title="Failed to extract transactions"
                supportingText={`Transactions could not be extracted on the following specified address: ${address}`}
                icon={<Warning />}
                type={InfoBoxType.Error}
                style={InfoBoxStyle.Elevated}
            />
        );
    }

    const tableColumns = generateTransactionsTableColumns();
    const hasTxns = data?.length > 0;

    if (!hasTxns) {
        return (
            <div className="flex h-20 items-center justify-center md:h-full">
                <span className="flex flex-row items-center gap-x-xs text-neutral-40 dark:text-neutral-60">
                    No transactions found
                </span>
            </div>
        );
    }

    return <TableCard data={data} columns={tableColumns} />;
}

export function TransactionsForAddress({ address }: TransactionsForAddressProps): JSX.Element {
    const client = useIotaClient();
    const indexerClient = useIotaIndexerClient();

    const { data, isPending, isError } = useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['transactions-for-address', address],
        queryFn: async () => {
            if (indexerClient) {
                // All-in-one query since indexer is available
                const results = await indexerClient.queryTransactionBlocks({
                    filter: { FromOrToAddress: { addr: address } },
                    order: 'descending',
                    limit: 100,
                    options: {
                        showInput: true,
                    },
                });

                return results?.data ?? [];
            } else {
                const filters = [{ ToAddress: address }, { FromAddress: address }];

                const results = await Promise.all(
                    filters.map((filter) =>
                        client.queryTransactionBlocks({
                            filter,
                            order: 'descending',
                            limit: 100,
                            options: {
                                showEffects: true,
                                showInput: true,
                            },
                        }),
                    ),
                );

                const inserted = new Set();
                const uniqueList: IotaTransactionBlockResponse[] = [];

                [...results[0].data, ...results[1].data]
                    .sort((a, b) => Number(b.timestampMs ?? 0) - Number(a.timestampMs ?? 0))
                    .forEach((txb) => {
                        if (inserted.has(txb.digest)) return;
                        uniqueList.push(txb);
                        inserted.add(txb.digest);
                    });

                return uniqueList;
            }
        },
    });

    return (
        <TransactionsForAddressTable
            data={data ?? []}
            isPending={isPending}
            isError={isError}
            address={address}
        />
    );
}
