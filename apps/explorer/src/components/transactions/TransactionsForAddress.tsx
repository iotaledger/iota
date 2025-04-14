// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { InfoBox, InfoBoxStyle, InfoBoxType, LoadingIndicator } from '@iota/apps-ui-kit';
import { useIotaClient } from '@iota/dapp-kit';
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

    const { data, isPending, isError } = useQuery({
        queryKey: ['transactions-for-address', address],
        queryFn: () =>
            client.queryTransactionBlocks({
                filter: { FromOrToAddress: { addr: address } },
                order: 'descending',
                options: {
                    showEffects: true,
                    showInput: true,
                },
            }),
        select: (response) => response.data,
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
