// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { PropsWithChildren, useState, createContext, useContext } from 'react';
import cx from 'classnames';
import {
    Button,
    ButtonSize,
    ButtonType,
    TableCell,
    TableCellProps,
    TableCellType,
    TableHeaderCell,
    TableHeaderCellProps,
} from '@/lib';
import { ArrowLeft, DoubleArrowLeft, ArrowRight, DoubleArrowRight } from '@iota/ui-icons';

export type TableProps = {
    /**
     * Does the table have pagination buttons.
     */
    hasPagination?: boolean;
    /**
     * On Next page button click.
     */
    onNextPageClick?: () => void;
    /**
     * On Previous page button click.
     */
    onPreviousPageClick?: () => void;
    /**
     * On First page button click.
     */
    onFirstPageClick?: () => void;
    /**
     * On Last page button click.
     */
    onLastPageClick?: () => void;
    /**
     * The label of the action button.
     */
    actionLabel?: string;
    /**
     * On Action button click.
     */
    onActionClick?: () => void;
    /**
     * The supporting label of the table.
     */
    supportingLabel?: string;
    /**
     * Whether the table has a checkbox column.
     */
    hasCheckboxColumn?: boolean;
    /**
     * The headers of the table.
     */
    headers: TableHeaderCellProps[];
    /**
     * The rows of the table.
     */
    rows: TableCellProps[][];
};

type TableContextProps = {
    hasCheckboxColumn: boolean;
    headerChecked: boolean;
    headerIndeterminate: boolean;
    handleHeaderCheckboxChange: (checked: boolean) => void;
    handleRowCheckboxChange: (rowIndex: number, checked: boolean) => void;
};

const TableContext = createContext<TableContextProps | undefined>(undefined);

export function Table({
    hasPagination,
    actionLabel,
    onNextPageClick,
    onPreviousPageClick,
    onFirstPageClick,
    onLastPageClick,
    onActionClick,
    supportingLabel,
    hasCheckboxColumn = false,
    headers,
    rows,
}: TableProps): JSX.Element {
    const [headerChecked, setHeaderChecked] = useState(false);
    const [headerIndeterminate, setHeaderIndeterminate] = useState(false);
    const [tableRows, setTableRows] = useState(
        rows.map(() => ({
            checked: false,
        })),
    );

    const handleHeaderCheckboxChange = (checked: boolean) => {
        setHeaderChecked(checked);
        setTableRows(tableRows.map((row) => ({ ...row, checked })));
        setHeaderIndeterminate(false);
    };

    const handleRowCheckboxChange = (rowIndex: number, checked: boolean) => {
        const updatedRows = tableRows.map((row, index) =>
            index === rowIndex ? { ...row, checked } : row,
        );
        setTableRows(updatedRows);

        const allChecked = updatedRows.every((row) => row.checked);
        const anyChecked = updatedRows.some((row) => row.checked);

        setHeaderChecked(allChecked);
        setHeaderIndeterminate(!allChecked && anyChecked);
    };

    const contextValue: TableContextProps = {
        hasCheckboxColumn,
        headerChecked,
        headerIndeterminate,
        handleHeaderCheckboxChange,
        handleRowCheckboxChange,
    };

    return (
        <TableContext.Provider value={contextValue}>
            <div className="w-full">
                <div className="overflow-auto">
                    <table className="w-full table-auto">
                        <TableHeader>
                            <TableRow>
                                {hasCheckboxColumn && (
                                    <TableCell
                                        type={TableCellType.Checkbox}
                                        isChecked={headerChecked}
                                        onChange={handleHeaderCheckboxChange}
                                        isIndeterminate={headerIndeterminate}
                                    />
                                )}
                                {headers.map((header, index) => (
                                    <TableHeaderCell key={index} {...header} />
                                ))}
                            </TableRow>
                        </TableHeader>
                        <TableBody>
                            {rows.map((row, rowIndex) => (
                                <TableRow key={rowIndex}>
                                    {hasCheckboxColumn && (
                                        <TableCell
                                            type={TableCellType.Checkbox}
                                            isChecked={tableRows[rowIndex].checked}
                                            onChange={(checked) =>
                                                handleRowCheckboxChange(rowIndex, checked)
                                            }
                                        />
                                    )}
                                    {row.map((cell, cellIndex) => (
                                        <TableCell key={cellIndex} {...cell} />
                                    ))}
                                </TableRow>
                            ))}
                        </TableBody>
                    </table>
                </div>
                <div
                    className={cx('flex w-full items-center justify-between gap-2 pt-md', {
                        hidden: !supportingLabel && !hasPagination && !actionLabel,
                    })}
                >
                    {hasPagination && (
                        <div className="flex gap-2">
                            <Button
                                type={ButtonType.Secondary}
                                size={ButtonSize.Small}
                                icon={<DoubleArrowLeft />}
                                onClick={onFirstPageClick}
                            />
                            <Button
                                type={ButtonType.Secondary}
                                size={ButtonSize.Small}
                                icon={<ArrowLeft />}
                                onClick={onPreviousPageClick}
                            />
                            <Button
                                type={ButtonType.Secondary}
                                size={ButtonSize.Small}
                                icon={<ArrowRight />}
                                onClick={onNextPageClick}
                            />
                            <Button
                                type={ButtonType.Secondary}
                                size={ButtonSize.Small}
                                icon={<DoubleArrowRight />}
                                onClick={onLastPageClick}
                            />
                        </div>
                    )}
                    {actionLabel && (
                        <div className="flex">
                            <Button
                                type={ButtonType.Secondary}
                                size={ButtonSize.Small}
                                text={actionLabel}
                                onClick={onActionClick}
                            />
                        </div>
                    )}
                    {supportingLabel && (
                        <span className="ml-auto text-label-md text-neutral-40 dark:text-neutral-60">
                            {supportingLabel}
                        </span>
                    )}
                </div>
            </div>
        </TableContext.Provider>
    );
}

export function useTableContext() {
    const context = useContext(TableContext);
    if (!context) {
        throw new Error('useTableContext must be used within a TableProvider');
    }
    return context;
}

export function TableHeader({ children }: PropsWithChildren): JSX.Element {
    return <thead>{children}</thead>;
}

export function TableRow({ children }: PropsWithChildren): JSX.Element {
    return <tr>{children}</tr>;
}

export function TableBody({ children }: PropsWithChildren): JSX.Element {
    return <tbody>{children}</tbody>;
}
