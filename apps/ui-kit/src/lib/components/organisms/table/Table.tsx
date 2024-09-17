// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { PropsWithChildren, ReactNode } from 'react';
import cx from 'classnames';
import { TableRowType, TableProvider, useTableContext, TableProviderProps } from './TableContext';
import {TableCellBase
    Button,
    ButtonProps,
    ButtonSize,
    ButtonType,
    Checkbox,
    TableBaseCell,
    TableHeaderCell,
} from '@/lib';
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
     * The action component..
     */
    action?: ReactNode;
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
    action,
    supportingLabel,
    onRowCheckboxChange,
    onHeaderCheckboxChange,
    rowIndexes,
    children,
}: PropsWithChildren<TableProps & TableProviderProps>): JSX.Element {
    return (
        <TableProvider
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
                        hidden: !supportingLabel && !paginationOptions && !action,
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
                    {action}
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

export function TableActionButton(props: PropsWithChildren<ButtonProps>) {
    return <Button type={ButtonType.Secondary} size={ButtonSize.Small} {...props} />;
}

export function TableHeader({ children }: PropsWithChildren): JSX.Element {
    return <thead>{children}</thead>;
}

export function TableRow({
    children,
    leading,
}: PropsWithChildren<{ leading?: React.JSX.Element }>): JSX.Element {
    return (
        <tr>
            {leading}
            {children}
        </tr>
    );
}

const TEXT_COLOR_CLASS = 'text-neutral-40 dark:text-neutral-60';
const TEXT_SIZE_CLASS = 'text-body-md';

export function TableBody({ children }: PropsWithChildren): JSX.Element {
    return <tbody className={cx(TEXT_COLOR_CLASS, TEXT_SIZE_CLASS)}>{children}</tbody>;
}

export function TableRowCheckbox({
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
TableCellBase
    return (
        <TableBaseCell isContentCentered>
            <Checkbox
                onCheckedChange={(event) => {
                    if (rowIndex !== undefined) {
                        toggleRowChecked?.(event.target.checked, rowIndex);
                    }
                }}
                isChecked={rowIndex !== undefined && rowsChecked.has(rowIndex)}
            />
        </TableCellBase>
    );
}
