// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import React from 'react';
import { BadgeType, Badge } from '../../atoms';
import { TableCellType } from './table-cell.enums';
import { Copy } from '@iota/ui-icons';
import cx from 'classnames';
interface TableCellBaseProps {
    /**
     * The label of the cell.
     */
    label: string;
    /**
     * If the cell is the last in the row and should not have a border.
     */
    hasLastBorderNoneClass?: boolean;
}

type TableCellText = {
    /**
     * The type of the cell.
     */
    type: TableCellType.Text;
    /**
     * The supporting label of the cell.
     */
    supportingLabel?: string;
};

type TableCellTextToCopy = {
    /**
     * The type of the cell.
     */
    type: TableCellType.TextToCopy;
    /**
     * The function to call when the copy icon is clicked.
     */
    onCopy?: () => void;
};

type TableCellBadge = {
    /**
     * The type of the cell.
     */
    type: TableCellType.Badge;
    /**
     * The type of the badge
     */
    badgeType: BadgeType;
};

type TableCellAvatarText = {
    /**
     * The type of the cell.
     */
    type: TableCellType.AvatarText;
    /**
     * The leading element of the cell.
     */
    leadingElement: React.JSX.Element;
};

export type TableCellProps = TableCellBaseProps &
    (TableCellText | TableCellTextToCopy | TableCellBadge | TableCellAvatarText);

export function TableCell(props: TableCellProps): JSX.Element {
    const { type, label, hasLastBorderNoneClass } = props;

    const textColorClass = 'text-neutral-40 dark:text-neutral-60';
    const textSizeClass = 'text-body-md';

    const Cell = () => {
        switch (type) {
            case TableCellType.Text:
                const { supportingLabel } = props;
                return (
                    <div className="flex flex-row items-baseline gap-1">
                        <span className={cx(textColorClass, textSizeClass)}>{label}</span>
                        {supportingLabel && (
                            <span className="text-body-sm text-neutral-60 dark:text-neutral-40">
                                {supportingLabel}
                            </span>
                        )}
                    </div>
                );
            case TableCellType.TextToCopy:
                const { onCopy } = props;
                return (
                    <div
                        className={cx('flex items-center space-x-2', textColorClass, textSizeClass)}
                    >
                        <span>{label}</span>
                        <Copy className="h-4 w-4 cursor-pointer" onClick={onCopy} />
                    </div>
                );
            case TableCellType.Badge:
                const { badgeType } = props;
                return <Badge type={badgeType} label={label} />;
            case TableCellType.AvatarText:
                const { leadingElement } = props;
                return (
                    <div className="flex items-center gap-x-2.5">
                        {leadingElement}
                        <span className={cx('text-label-lg', textColorClass)}>{label}</span>
                    </div>
                );
            default:
                return null;
        }
    };

    return (
        <td
            className={cx(
                'inline-flex h-14 flex-row items-center border-b border-shader-neutral-light-8 px-md dark:border-shader-neutral-dark-8',
                { 'last:border-none': hasLastBorderNoneClass },
            )}
        >
            <Cell />
        </td>
    );
}
