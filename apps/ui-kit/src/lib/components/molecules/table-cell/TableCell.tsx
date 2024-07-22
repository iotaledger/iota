// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import React from 'react';
import { BadgeType, Badge } from '../../atoms';
import { TableCellType } from './table-cell.enums';
import { Copy } from '@iota/ui-icons';
import cx from 'classnames';

interface TableCellProps {
    /**
     * The type of cell to render.
     */
    type?: TableCellType;
    /**
     * The text to display.
     */
    label: string;
    /**
     * The badge type to display.
     */
    badgeType?: BadgeType;
    /**
     * The leading element to display.
     */
    leadingElement?: React.JSX.Element;
    /**
     * The supporting label to display.
     */
    supportingLabel?: string;
    /**
     * If the cell has the last border none class.
     */
    hasLastBorderNoneClass?: boolean;
    /**
     * The onCopy event of the cell (optional).
     */
    onCopy?: (e: React.MouseEvent<SVGElement>) => void;
}

export function TableCell({
    type = TableCellType.Text,
    label,
    badgeType = BadgeType.PrimarySolid,
    leadingElement,
    supportingLabel,
    hasLastBorderNoneClass,
    onCopy,
}: TableCellProps): React.JSX.Element {
    const textColorClass = 'text-neutral-40 dark:text-neutral-60';
    const textSizeClass = 'text-body-md';
    const Cell = () => {
        switch (type) {
            case TableCellType.Text:
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
                return (
                    <div
                        className={cx('flex items-center space-x-2', textColorClass, textSizeClass)}
                    >
                        <span>{label}</span>
                        <Copy className="h-4 w-4 cursor-pointer" onClick={onCopy} />
                    </div>
                );
            case TableCellType.Badge:
                return <Badge type={badgeType} label={label} />;
            case TableCellType.AvatarText:
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
