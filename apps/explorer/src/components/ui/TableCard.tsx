// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    Table,
    TableBody,
    TableBodyRow,
    TableCell,
    type TableCellProps,
    TableHeader,
    TableHeaderCell,
    TableHeaderRow,
    type TablePaginationOptions,
} from '@iota/apps-ui-kit';
import {
    type ColumnDef,
    flexRender,
    getCoreRowModel,
    getSortedRowModel,
    type SortingState,
    useReactTable,
} from '@tanstack/react-table';
import clsx from 'clsx';
import { useMemo, useState } from 'react';
import { useNavigateWithQuery } from './LinkWithQuery';

export interface TableCardProps<DataType extends object> {
    refetching?: boolean;
    data: DataType[];
    columns: ColumnDef<DataType>[];
    sortTable?: boolean;
    defaultSorting?: SortingState;
    paginationOptions?: TablePaginationOptions;
    totalLabel?: string;
    viewAll?: string;
}

export function TableCard<DataType extends object>({
    refetching,
    data,
    columns,
    sortTable,
    defaultSorting,
    paginationOptions,
    totalLabel,
    viewAll,
}: TableCardProps<DataType>): JSX.Element {
    const navigate = useNavigateWithQuery();
    const [sorting, setSorting] = useState<SortingState>(defaultSorting || []);

    console.log("bfore", data, columns)
    const table = useReactTable({
        data,
        columns,
        getCoreRowModel: getCoreRowModel(),
        getSortedRowModel: getSortedRowModel(),
        onSortingChange: setSorting,
        enableSorting: !!sortTable,
        enableSortingRemoval: false,
        initialState: {
            sorting,
        },
        state: {
            sorting,
        },
    });
    console.log("after")

    return (
        <div className={clsx('w-full overflow-x-auto', refetching && 'opacity-50')}>
            <Table
                rowIndexes={table.getRowModel().rows.map((row) => row.index)}
                paginationOptions={paginationOptions}
                actionLabel={viewAll ? 'View All' : undefined}
                supportingLabel={totalLabel}
                onActionClick={
                    viewAll
                        ? () => {
                              navigate(viewAll, {});
                          }
                        : undefined
                }
            >
                <TableHeader>
                    {table.getHeaderGroups().map((headerGroup) => (
                        <TableHeaderRow key={headerGroup.id}>
                            {headerGroup.headers.map(({ id, column }) => (
                                <TableHeaderCell
                                    key={id}
                                    columnKey={id}
                                    label={column.columnDef.header?.toString()}
                                    hasSort={column.columnDef.enableSorting}
                                    onSortClick={
                                        column.columnDef.enableSorting
                                            ? column.getToggleSortingHandler()
                                            : undefined
                                    }
                                />
                            ))}
                        </TableHeaderRow>
                    ))}
                </TableHeader>
                <TableBody>
                    {table.getRowModel().rows.map((row) => (
                        <TableBodyRow key={row.id} rowIndex={row.index}>
                            {row.getVisibleCells().map((cell) => (
                                <td key={cell.id}>
                                    {flexRender(cell.column.columnDef.cell, cell.getContext())}
                                </td>
                            ))}
                        </TableBodyRow>
                    ))}
                </TableBody>
            </Table>
        </div>
    );
}
