// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { PropsWithChildren, useEffect } from 'react';
import cx from 'classnames';
import { TableRowType, TableProvider, useTableContext } from './TableContext';
import { Button, ButtonSize, ButtonType, TableCell, TableCellType, TableHeaderCell } from '@/lib';
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
};

export function Table({
    hasPagination,
    actionLabel,
    onNextPageClick,
    onPreviousPageClick,
    onFirstPageClick,
    onLastPageClick,
    onActionClick,
    supportingLabel,
    hasCheckboxColumn,
    children,
}: PropsWithChildren<TableProps>): JSX.Element {
    return (
        <TableProvider hasCheckboxColumn={hasCheckboxColumn}>
            <div className="w-full">
                <div className="overflow-auto">
                    <table className="w-full table-auto">{children}</table>
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
    const { hasCheckboxColumn, toggleRowChecked } = useTableContext();

    useEffect(() => {
        if (rowIndex !== undefined && rowIndex !== null) {
            toggleRowChecked?.(rowIndex, false);
        }
    }, [toggleRowChecked, rowIndex]);

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
        headerChecked,
        isHeaderIndeterminate,
    } = useTableContext();

    if (type === TableRowType.Header) {
        return (
            <TableHeaderCell
                isCellContentCentered
                hasCheckbox
                onCheckboxChange={toggleHeaderChecked}
                isChecked={headerChecked}
                columnKey={1}
                isIndeterminate={isHeaderIndeterminate}
            />
        );
    }

    return (
        <TableCell
            isCellContentCentered
            onChange={(checked) => rowIndex !== undefined && toggleRowChecked?.(rowIndex, checked)}
            type={TableCellType.Checkbox}
            isChecked={rowIndex !== undefined && rowsChecked?.[rowIndex]}
        />
    );
}
