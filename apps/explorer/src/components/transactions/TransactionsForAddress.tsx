// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    DropdownPosition,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    LoadingIndicator,
    Select,
    SelectSize,
    type TablePaginationOptions,
} from '@iota/apps-ui-kit';
import { useIotaClient } from '@iota/dapp-kit';
import { type IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { Warning } from '@iota/apps-ui-icons';
import { useInfiniteQuery } from '@tanstack/react-query';
import { TableCard } from '~/components/ui';
import { generateTransactionsTableColumns } from '~/lib/ui';
import { useState } from 'react';
import { PAGE_SIZES_RANGE_10_50 } from '~/lib';
import { useCursorPagination } from '@iota/core';
import clsx from 'clsx';

const PAGE_RANGE = PAGE_SIZES_RANGE_10_50;

interface TransactionsForAddressProps {
    address: string;
}

interface TransactionsForAddressTableProps {
    data: IotaTransactionBlockResponse[];
    isPending: boolean;
    isError: boolean;
    address: string;
    limit: number;
    setLimit: (limit: number) => void;
    pagination: TablePaginationOptions;
    isFetchingNextPage: boolean;
}

export function TransactionsForAddressTable({
    data,
    isPending,
    isError,
    address,
    limit,
    setLimit,
    pagination,
    isFetchingNextPage,
}: TransactionsForAddressTableProps): JSX.Element {
    if (isPending || isFetchingNextPage) {
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

    return (
        <TableCard
            data={data}
            columns={tableColumns}
            paginationOptions={pagination}
            pageSizeSelector={
                <Select
                    value={limit.toString()}
                    options={PAGE_RANGE.map((size) => ({
                        label: `${size} / page`,
                        id: size.toString(),
                    }))}
                    size={SelectSize.Small}
                    dropdownPosition={DropdownPosition.Top}
                    onValueChange={(e) => {
                        setLimit(Number(e));
                        pagination.onFirst?.();
                    }}
                />
            }
        />
    );
}

export function TransactionsForAddress({ address }: TransactionsForAddressProps): JSX.Element {
    const [limit, setLimit] = useState(PAGE_RANGE[0]);
    const client = useIotaClient();

    const transactions = useInfiniteQuery({
        queryKey: ['transactions-for-address', address, limit],
        queryFn: ({ pageParam: cursor }) =>
            client.queryTransactionBlocks({
                filter: { FromOrToAddress: { addr: address } },
                order: 'descending',
                options: {
                    showEffects: true,
                    showInput: true,
                },
                cursor,
                limit,
            }),
        initialPageParam: null as string | null,
        getNextPageParam: (lastPage) =>
            lastPage.hasNextPage ? (lastPage.nextCursor ?? null) : null,
    });

    const { data, isPending, isFetchingNextPage, isError, pagination, hasNextPage } =
        useCursorPagination(transactions);

    return (
        <div
            className={clsx(
                hasNextPage && limit === 10 && 'min-h-[700px]',
                hasNextPage && limit === 20 && 'min-h-[1240px]',
                hasNextPage && limit === 30 && 'min-h-[1800px]',
                hasNextPage && limit === 40 && 'min-h-[2360px]',
                hasNextPage && limit === 50 && 'min-h-[2920px]',
            )}
        >
            <TransactionsForAddressTable
                data={data?.data ?? []}
                isPending={isPending}
                isError={isError}
                address={address}
                limit={limit}
                setLimit={setLimit}
                pagination={pagination}
                isFetchingNextPage={isFetchingNextPage}
            />
        </div>
    );
}
