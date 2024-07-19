// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import React from 'react';
import { BadgeType, Badge } from '../../atoms';
import { TableCellType } from './table-cell.enums';
import { Copy } from '@iota/ui-icons';
import cx from 'classnames';

interface TableCellProps {
    type: TableCellType;
    label: string;
    badgeType?: BadgeType;
    leadingElement?: React.JSX.Element;
    supportingLabel?: string;
}

export function TableCell({
    type,
    label,
    badgeType = BadgeType.PrimarySolid,
    leadingElement,
    supportingLabel,
}: TableCellProps): React.JSX.Element {
    const textColorClass = 'text-neutral-40 dark:text-neutral-60';
    const textSizeClass = 'text-body-md';
    const renderCell = () => {
        switch (type) {
            case TableCellType.Text:
                return (
                    <div className="flex flex-row items-baseline gap-0.5">
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
                        <Copy />
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
        <td className="border-b border-shader-neutral-light-8 p-md dark:border-shader-neutral-dark-8">
            {renderCell()}
        </td>
    );
}
