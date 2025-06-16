// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    Table,
    TableBody,
    TableHeader,
    TableHeaderCell,
    TableRow,
    TableActionButton,
    type TablePaginationOptions,
    TableHeaderCellSortOrder,
} from '@iota/apps-ui-kit';
import {
    type ColumnDef,
    flexRender,
    getCoreRowModel,
    getSortedRowModel,
    type RowData,
    type SortingState,
    useReactTable,
} from '@tanstack/react-table';
import clsx from 'clsx';
import { Fragment, type ReactNode, useState } from 'react';
import { Link } from './Link';

export interface TableCardProps<DataType extends RowData> {
    refetching?: boolean;
    data: DataType[];
    columns: ColumnDef<DataType>[];
    sortTable?: boolean;
    defaultSorting?: SortingState;
    areHeadersCentered?: boolean;
    paginationOptions?: TablePaginationOptions;
    totalLabel?: string;
    viewAll?: string;
    pageSizeSelector?: ReactNode;
    heightFull?: boolean;
}

export function TableCard<DataType extends object>({
    refetching,
    data,
    columns,
    sortTable,
    defaultSorting,
    areHeadersCentered,
    paginationOptions,
    totalLabel,
    viewAll,
    pageSizeSelector,
    heightFull,
}: TableCardProps<DataType>): JSX.Element {
    const [sorting, setSorting] = useState<SortingState>(defaultSorting || []);

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

    function getColumnSortOrder(columnId: string, sortEnabled?: boolean) {
        const sortState = sorting.find((sort) => sort.id === columnId);
        if (!sortEnabled || !sortState) {
            return undefined;
        }

        if (sortState) {
            return sortState.desc ? TableHeaderCellSortOrder.Desc : TableHeaderCellSortOrder.Asc;
        }
    }

    return (
        <div
            className={clsx(
                'w-full overflow-visible',
                refetching && 'opacity-50',
                heightFull && 'h-full',
            )}
        >
            <Table
                heightFull
                rowIndexes={table.getRowModel().rows.map((row) => row.index)}
                paginationOptions={paginationOptions}
                supportingLabel={totalLabel}
                action={
                    viewAll ? (
                        <Link to={viewAll}>
                            <TableActionButton text="View All" />
                        </Link>
                    ) : undefined
                }
                pageSizeSelector={pageSizeSelector}
            >
                <TableHeader>
                    {table.getHeaderGroups().map((headerGroup) => (
                        <TableRow key={headerGroup.id}>
                            {headerGroup.headers.map(({ id, column }) => (
                                <TableHeaderCell
                                    key={id}
                                    columnKey={id}
                                    label={column.columnDef.header?.toString()}
                                    hasSort={column.columnDef.enableSorting}
                                    sortOrder={getColumnSortOrder(
                                        id,
                                        column.columnDef.enableSorting,
                                    )}
                                    onSortClick={(key, sortOrder) => {
                                        setSorting([
                                            {
                                                id: String(key),
                                                desc: sortOrder === TableHeaderCellSortOrder.Desc,
                                            },
                                        ]);
                                        column.columnDef.enableSorting
                                            ? column.getToggleSortingHandler()
                                            : undefined;
                                    }}
                                    isContentCentered={areHeadersCentered}
                                />
                            ))}
                        </TableRow>
                    ))}
                </TableHeader>
                <TableBody>
                    {table.getRowModel().rows.map((row) => (
                        <TableRow key={row.id}>
                            {row.getVisibleCells().map((cell) => (
                                <Fragment key={cell.id}>
                                    {flexRender(cell.column.columnDef.cell, cell.getContext())}
                                </Fragment>
                            ))}
                        </TableRow>
                    ))}
                </TableBody>
            </Table>
        </div>
    );
}
