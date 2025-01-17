// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';
import { SortByDown, SortByUp } from '@iota/apps-ui-icons';
import cx from 'classnames';
import { Checkbox } from '@/lib';
import { TableHeaderCellSortOrder } from './tableHeaderCell.enums';

export interface TableHeaderCellProps {
    /**
     * The column key.
     */
    columnKey: string | number;
    /**
     * The label of the Header cell.
     */
    label?: string;
    /**
     * Action component to be rendered on the left side.
     */
    actionLeft?: React.ReactNode;
    /**
     * Action component to be rendered on the right side.
     */
    actionRight?: React.ReactNode;
    /**
     * Has Sort icon.
     */
    hasSort?: boolean;
    /**
     * On Sort icon click.
     */
    onSortClick?: (columnKey: string | number, sortOrder: TableHeaderCellSortOrder) => void;
    /**
     * Has Checkbox.
     */
    hasCheckbox?: boolean;
    /**
     * Is Checkbox checked.
     */
    isChecked?: boolean;
    /**
     * Is Checkbox indeterminate.
     */
    isIndeterminate?: boolean;
    /**
     * On Checkbox change.
     */
    onCheckboxChange?: (e: React.ChangeEvent<HTMLInputElement>) => void;
    /**
     * Whether the cell content should be centered.
     */
    isContentCentered?: boolean;
}

export function TableHeaderCell({
    label,
    columnKey,
    hasSort,
    hasCheckbox,
    isChecked,
    isIndeterminate,
    isContentCentered,
    onSortClick,
    onCheckboxChange,
}: TableHeaderCellProps): JSX.Element {
    const [sortOrder, setSortOrder] = useState<TableHeaderCellSortOrder | null>(
        TableHeaderCellSortOrder.Asc,
    );

    const handleSort = () => {
        const newSortOrder =
            sortOrder === TableHeaderCellSortOrder.Asc
                ? TableHeaderCellSortOrder.Desc
                : TableHeaderCellSortOrder.Asc;
        setSortOrder(newSortOrder);
        if (onSortClick) {
            onSortClick(columnKey, newSortOrder);
        }
    };

    const textColorClass = 'text-neutral-10 dark:text-neutral-92';
    const textSizeClass = 'text-label-lg';

    return (
        <th
            className={cx(
                'state-layer relative h-14 border-b border-shader-neutral-light-8 px-md after:pointer-events-none dark:border-shader-neutral-dark-8',
            )}
        >
            <div
                className={cx(
                    'flex flex-row items-center gap-1 [&_svg]:h-4 [&_svg]:w-4',
                    textColorClass,
                    textSizeClass,
                    {
                        'justify-center': isContentCentered,
                    },
                )}
            >
                {hasCheckbox ? (
                    <Checkbox
                        isChecked={isChecked}
                        isIndeterminate={isIndeterminate}
                        onCheckedChange={onCheckboxChange}
                    />
                ) : (
                    <span
                        className={cx({
                            'text-left': !isContentCentered,
                        })}
                    >
                        {label}
                    </span>
                )}
                {hasSort && sortOrder === TableHeaderCellSortOrder.Asc && (
                    <SortByUp className="cursor-pointer" onClick={handleSort} />
                )}
                {hasSort && sortOrder === TableHeaderCellSortOrder.Desc && (
                    <SortByDown className="cursor-pointer" onClick={handleSort} />
                )}
            </div>
        </th>
    );
}
