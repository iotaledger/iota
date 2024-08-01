// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import React, { useState } from 'react';
import { SortByDown, SortByUp } from '@iota/ui-icons';
import cx from 'classnames';
import { Checkbox } from '@/lib';

export interface TableHeaderCellProps {
    /**
     * The label of the Header cell.
     */
    label: string;
    /**
     * Has Sort icon.
     */
    hasSort?: boolean;
    /**
     * On Sort icon click.
     */
    onSortClick?: (order: 'asc' | 'desc') => void;
    /**
     * Has Checkbox.
     */
    hasCheckbox?: boolean;
    /**
     * On Checkbox click.
     */
    onCheckboxClick?: () => void;
}

export function TableHeaderCell({
    label,
    hasSort,
    hasCheckbox,
    onSortClick,
    onCheckboxClick,
}: TableHeaderCellProps): JSX.Element {
    const [sortOrder, setSortOrder] = useState<'asc' | 'desc' | null>('asc');

    const handleSort = () => {
        const newSortOrder = sortOrder === 'asc' ? 'desc' : 'asc';
        setSortOrder(newSortOrder);
        if (onSortClick) {
            onSortClick(newSortOrder);
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
            <div className={cx('flex flex-row items-center gap-1', textColorClass, textSizeClass)}>
                {hasCheckbox && <Checkbox onChange={onCheckboxClick} />}
                <span>{label}</span>
                {hasSort && sortOrder === 'asc' && (
                    <SortByUp className="ml-auto h-4 w-4 cursor-pointer" onClick={handleSort} />
                )}
                {hasSort && sortOrder === 'desc' && (
                    <SortByDown className="ml-auto h-4 w-4 cursor-pointer" onClick={handleSort} />
                )}
            </div>
        </th>
    );
}
