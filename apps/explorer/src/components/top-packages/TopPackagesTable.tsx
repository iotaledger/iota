// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type MoveCallMetric } from '@iota/iota-sdk/client';
import { Text } from '@iota/ui';
import { useMemo } from 'react';

import { ObjectLink, PlaceholderTable, TableCard } from '~/components/ui';

interface TopPackagesTableProps {
    data: MoveCallMetric[];
    isLoading: boolean;
}

function TxnCountHeader(): JSX.Element {
    return (
        <div className="w-full text-right">
            <Text variant="bodySmall/medium">Transactions</Text>
        </div>
    );
}

export function TopPackagesTable({ data, isLoading }: TopPackagesTableProps) {
    const tableData = useMemo(
        () => ({
            data: data?.map(([item, count]) => ({
                module: (
                    <ObjectLink
                        label={item.module}
                        objectId={`${item.package}?module=${item.module}`}
                    />
                ),
                function: <Text variant="bodySmall/medium">{item.function}</Text>,
                package: <ObjectLink objectId={item.package} />,
                count: (
                    <div className="text-right">
                        <Text mono variant="body/medium">
                            {Number(count).toLocaleString()}
                        </Text>
                    </div>
                ),
            })),
            columns: [
                {
                    header: 'Package ID',
                    accessorKey: 'package',
                },
                {
                    header: 'Module',
                    accessorKey: 'module',
                },
                {
                    header: 'Function',
                    accessorKey: 'function',
                },
                {
                    header: TxnCountHeader,
                    accessorKey: 'count',
                },
            ],
        }),
        [data],
    );

    if (isLoading) {
        return (
            <PlaceholderTable
                colHeadings={['Module', 'Function', 'Package ID', 'Count']}
                rowCount={10}
                rowHeight="15px"
            />
        );
    }

    return <TableCard data={tableData.data} columns={tableData.columns} />;
}
