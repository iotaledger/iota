// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TableCellText, TableCellBase } from '@iota/apps-ui-kit';
import { type MoveCallMetric } from '@iota/iota-sdk/client';
import { type ColumnDef } from '@tanstack/react-table';

import { ObjectLink, PlaceholderTable, TableCard } from '~/components/ui';
import { onCopySuccess } from '~/lib/utils';

interface TopPackagesTableProps {
    data: MoveCallMetric[];
    isLoading: boolean;
}

const tableColumns: ColumnDef<MoveCallMetric>[] = [
    {
        header: 'Module',
        id: 'module',
        cell({ row: { original: metric } }) {
            const item = metric[0];
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
        id: 'function',
        cell({ row: { original: metric } }) {
            const item = metric[0];
            return (
                <TableCellBase>
                    <TableCellText>{item.function}</TableCellText>
                </TableCellBase>
            );
        },
    },
    {
        header: 'Package',
        id: 'package',
        cell({ row: { original: metric } }) {
            const item = metric[0].package;
            return (
                <TableCellBase>
                    <ObjectLink objectId={item} copyText={item} onCopySuccess={onCopySuccess}>
                        <TableCellText>{item}</TableCellText>
                    </ObjectLink>
                </TableCellBase>
            );
        },
    },
    {
        header: 'Count',
        id: 'count',
        cell({ row: { original: metric } }) {
            const item = metric[1];
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
