// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TableCellType } from '@iota/apps-ui-kit';
import { type MoveCallMetric } from '@iota/iota-sdk/client';
import { useMemo } from 'react';

import { createLinkTo, objectToLink, PlaceholderTable, TableCard } from '~/components/ui';

interface TopPackagesTableProps {
    data: MoveCallMetric[];
    isLoading: boolean;
}

export function TopPackagesTable({ data, isLoading }: TopPackagesTableProps) {
    const tableData = useMemo(
        () => ({
            data: data?.map(([item, count]) => ({
                module: {
                    type: TableCellType.Link,
                    label: item.module,
                    to: createLinkTo(item.package, 'module')({ module: item.module }),
                },
                function: {
                    type: TableCellType.Text,
                    label: item.function,
                },
                package: {
                    type: TableCellType.Link,
                    label: item.package,
                    to: objectToLink({ objectId: item.package }),
                },
                count: {
                    type: TableCellType.Text,
                    label: Number(count).toLocaleString(),
                },
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
                    header: 'Transactions',
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
