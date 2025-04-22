// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import type { PropsWithChildren } from 'react';
import cx from 'classnames';
import { Placeholder } from '../../atoms';

interface TableCellBaseProps {
    /**
     * The label of the cell.
     */
    label?: string;
    /**
     * If the cell is the last in the row and should not have a border.
     */
    hasLastBorderNoneClass?: boolean;
    /**
     * Whether the cell content should be centered.
     */
    isContentCentered?: boolean;
}

export function TableCellBase({
    children,
    hasLastBorderNoneClass,
    isContentCentered,
}: PropsWithChildren<TableCellBaseProps>) {
    return (
        <td
            className={cx(
                'h-14 border-b border-shader-neutral-light-8 px-md dark:border-shader-neutral-dark-8',
                { 'last:border-none': hasLastBorderNoneClass },
                { 'flex items-center justify-center': isContentCentered },
            )}
        >
            {children}
        </td>
    );
}

export interface TableCellTextProps {
    supportingLabel?: string;
}

export function TableCellText({
    children,
    supportingLabel,
}: PropsWithChildren<TableCellTextProps>) {
    return (
        <div className="flex flex-row items-baseline gap-1">
            <span>{children}</span>
            {supportingLabel && (
                <span className="text-body-sm text-neutral-60 dark:text-neutral-40">
                    {supportingLabel}
                </span>
            )}
        </div>
    );
}

export function TableCellPlaceholder() {
    return <Placeholder />;
}
