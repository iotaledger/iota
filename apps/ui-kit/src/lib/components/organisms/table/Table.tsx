// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { PropsWithChildren } from 'react';
import cx from 'classnames';
import { TableRowType, TableProvider, useTableContext, TableProviderProps } from './TableContext';
import { Button, ButtonSize, ButtonType, TableCell, TableCellType, TableHeaderCell } from '@/lib';
import { ArrowLeft, DoubleArrowLeft, ArrowRight, DoubleArrowRight } from '@iota/ui-icons';

export interface TablePaginationOptions {
    /**
     * On Next page button click.
     */
    onNext?: () => void;
    /**
     * On Previous page button click.
     */
    onPrev?: () => void;
    /**
     * On First page button click.
     */
    onFirst?: () => void;
    /**
     * On Last page button click.
     */
    onLast?: () => void;
    /**
     * Has Next button.
     */
    hasNext?: boolean;
    /**
     * Has Previous button.
     */
    hasPrev?: boolean;
    /**
     * Has First button.
     */
    hasFirst?: boolean;
    /**
     * Has Last button.
     */
    hasLast?: boolean;
}

export type TableProps = {
    /**
     * Options for the table pagination.
     */
    paginationOptions?: TablePaginationOptions;
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
     * Numeric indexes of all the rows.
     */
    rowIndexes: number[];
};

export function Table({
    paginationOptions,
    actionLabel,
    onActionClick,
    supportingLabel,
    hasCheckboxColumn,
    onRowCheckboxChange,
    onHeaderCheckboxChange,
    rowIndexes,
    children,
}: PropsWithChildren<TableProps & TableProviderProps>): JSX.Element {
    return (
        <TableProvider
            hasCheckboxColumn={hasCheckboxColumn}
            onRowCheckboxChange={onRowCheckboxChange}
            onHeaderCheckboxChange={onHeaderCheckboxChange}
            rowIndexes={rowIndexes}
        >
            <div className="w-full">
                <div className="overflow-auto">
                    <table className="w-full table-auto">{children}</table>
                </div>
                <div
                    className={cx('flex w-full items-center justify-between gap-2 pt-md', {
                        hidden: !supportingLabel && !paginationOptions && !actionLabel,
                    })}
                >
                    {paginationOptions && (
                        <div className="flex gap-2">
                            <Button
                                type={ButtonType.Secondary}
                                size={ButtonSize.Small}
                                icon={<DoubleArrowLeft />}
                                disabled={!paginationOptions.hasFirst}
                                onClick={paginationOptions.onFirst}
                            />
                            <Button
                                type={ButtonType.Secondary}
                                size={ButtonSize.Small}
                                icon={<ArrowLeft />}
                                disabled={!paginationOptions.hasPrev}
                                onClick={paginationOptions.onPrev}
                            />
                            <Button
                                type={ButtonType.Secondary}
                                size={ButtonSize.Small}
                                icon={<ArrowRight />}
                                disabled={!paginationOptions.hasNext}
                                onClick={paginationOptions.onNext}
                            />
                            <Button
                                type={ButtonType.Secondary}
                                size={ButtonSize.Small}
                                icon={<DoubleArrowRight />}
                                disabled={!paginationOptions.hasLast}
                                onClick={paginationOptions.onLast}
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
        </TableProvider>
    );
}

export function TableHeader({ children }: PropsWithChildren): JSX.Element {
    return <thead>{children}</thead>;
}

export function TableHeaderRow({ children }: PropsWithChildren): JSX.Element {
    return <TableRow type={TableRowType.Header}>{children}</TableRow>;
}

export function TableBodyRow({
    children,
    rowIndex,
}: PropsWithChildren<{ rowIndex: number }>): JSX.Element {
    return (
        <TableRow type={TableRowType.Body} rowIndex={rowIndex}>
            {children}
        </TableRow>
    );
}

function TableRow({
    children,
    rowIndex,
    type = TableRowType.Body,
}: PropsWithChildren<{ rowIndex?: number; type: TableRowType }>): JSX.Element {
    const { hasCheckboxColumn } = useTableContext();

    return (
        <tr>
            {hasCheckboxColumn && <TableRowCheckbox type={type} rowIndex={rowIndex} />}
            {children}
        </tr>
    );
}

export function TableBody({ children }: PropsWithChildren): JSX.Element {
    return <tbody>{children}</tbody>;
}

function TableRowCheckbox({
    type,
    rowIndex,
}: {
    type: TableRowType;
    rowIndex?: number;
}): React.JSX.Element {
    const {
        toggleHeaderChecked,
        toggleRowChecked,
        rowsChecked,
        isHeaderChecked,
        isHeaderIndeterminate,
    } = useTableContext();

    if (type === TableRowType.Header) {
        return (
            <TableHeaderCell
                isContentCentered
                hasCheckbox
                onCheckboxChange={(event) => {
                    toggleHeaderChecked(event.target.checked);
                }}
                isChecked={isHeaderChecked}
                columnKey={1}
                isIndeterminate={isHeaderIndeterminate}
            />
        );
    }

    return (
        <TableCell
            isContentCentered
            onChange={(event) => {
                if (rowIndex !== undefined) {
                    toggleRowChecked?.(event.target.checked, rowIndex);
                }
            }}
            type={TableCellType.Checkbox}
            isChecked={rowIndex !== undefined && rowsChecked.has(rowIndex)}
        />
    );
}
