// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TableCellText, TableCellBase } from '@iota/apps-ui-kit';
import { type MoveCallMetric } from '@iota/iota-sdk/client';
import { type ColumnDef } from '@tanstack/react-table';

import { ObjectLink, PlaceholderTable, TableCard } from '~/components/ui';

interface TopPackagesTableProps {
    data: MoveCallMetric[];
    isLoading: boolean;
}

const tableColumns: ColumnDef<MoveCallMetric>[] = [
    {
        header: 'Module',
        accessorKey: 'item[0]',
        cell({ getValue }) {
            const item = getValue<MoveCallMetric[0]>();
            return (
                <TableCellBase>
                    <ObjectLink objectId={`${item.package}?module=${item.module}`}>
                        <TableCellText>{item.module}</TableCellText>
                    </ObjectLink>
                </TableCellBase>
            );
        },
    },
    {
        header: 'Function',
        accessorKey: 'item[0].function',
        cell({ getValue }) {
            const item = getValue<MoveCallMetric[0]['function']>();
            return (
                <TableCellBase>
                    <TableCellText>{item}</TableCellText>
                </TableCellBase>
            );
        },
    },
    {
        header: 'Function',
        accessorKey: 'item[0].package',
        cell({ getValue }) {
            const item = getValue<MoveCallMetric[0]['package']>();
            return (
                <TableCellBase>
                    <ObjectLink objectId={item}>
                        <TableCellText>{item}</TableCellText>
                    </ObjectLink>
                </TableCellBase>
            );
        },
    },
    {
        header: 'Count',
        accessorKey: 'item[1]',
        cell({ getValue }) {
            const item = getValue<MoveCallMetric[1]>();
            return (
                <TableCellBase>
                    <TableCellText>{item}</TableCellText>
                </TableCellBase>
            );
        },
    },
];

export function TopPackagesTable({ data, isLoading }: TopPackagesTableProps) {
    if (isLoading) {
        return (
            <PlaceholderTable
                colHeadings={['Module', 'Function', 'Package ID', 'Count']}
                rowCount={10}
                rowHeight="15px"
            />
        );
    }

    return <TableCard data={data} columns={tableColumns} />;
}
