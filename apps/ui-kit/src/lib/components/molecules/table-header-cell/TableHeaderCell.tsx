// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SortByDown, SortByUp } from '@iota/apps-ui-icons';
import cx from 'classnames';
import { Checkbox } from '@/components/atoms/checkbox';
import { TableHeaderCellSortOrder } from './tableHeaderCell.enums';

export interface TableHeaderCellProps extends React.AriaAttributes {
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
    /**
     * Sort order when cell is initialized
     */
    sortOrder?: TableHeaderCellSortOrder;
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
    sortOrder,
    ...ariaProps
}: TableHeaderCellProps): JSX.Element {
    const handleSort = () => {
        const newSortOrder =
            sortOrder === TableHeaderCellSortOrder.Asc
                ? TableHeaderCellSortOrder.Desc
                : TableHeaderCellSortOrder.Asc;
        if (onSortClick) {
            onSortClick(columnKey, newSortOrder);
        }
    };

    const textColorClass = 'table-header-text-color';
    const textSizeClass = 'text-label-lg';

    const sortElement = (() => {
        if (!hasSort) {
            return null;
        }

        if (sortOrder === TableHeaderCellSortOrder.Asc) {
            return <SortByUp className="shrink-0" />;
        }

        if (sortOrder === TableHeaderCellSortOrder.Desc) {
            return <SortByDown className="shrink-0" />;
        }

        return <SortByUp className="invisible shrink-0 group-hover:visible" />;
    })();

    return (
        <th
            onClick={hasSort ? handleSort : undefined}
            className={cx(
                'state-layer table-cell-border-color group relative h-14 border-b px-md after:pointer-events-none',
                {
                    'cursor-pointer': hasSort,
                },
            )}
            aria-sort={
                hasSort
                    ? sortOrder === TableHeaderCellSortOrder.Asc
                        ? 'ascending'
                        : sortOrder === TableHeaderCellSortOrder.Desc
                          ? 'descending'
                          : 'none'
                    : undefined
            }
            {...ariaProps}
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
                {sortElement}
            </div>
        </th>
    );
}
