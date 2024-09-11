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

    // Use Columns to create a table
    const processedcol = useMemo<ColumnDef<DataType>[]>(
        () =>
            columns.map((column) => ({
                ...column,
                // cell renderer for each column from react-table
                // cell should be in the column definition
                //TODO: move cell to column definition
                ...(!sortTable && { cell: ({ getValue }) => getValue() }),
            })),
        [columns, sortTable],
    );

    const table = useReactTable({
        data,
        columns: processedcol,
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
                                <TableCell key={cell.id} {...cell.getValue<TableCellProps>()} />
                            ))}
                        </TableBodyRow>
                    ))}
                </TableBody>
            </Table>
        </div>
    );
}
