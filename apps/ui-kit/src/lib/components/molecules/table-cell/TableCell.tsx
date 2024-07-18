// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import React from 'react';
import { BadgeType, Badge } from '../../atoms';
import { TableCellType } from './table-cell.enums';
import { Copy } from '@iota/ui-icons';

interface TableCellProps {
    type: TableCellType;
    label: string;
    badgeType?: BadgeType;
    leadingElement?: React.JSX.Element;
    toggleChecked?: boolean;
    onToggleChange?: (checked: boolean) => void;
}

export function TableCell({
    type,
    label,
    badgeType = BadgeType.PrimarySolid,
    leadingElement,
    toggleChecked,
    onToggleChange,
}: TableCellProps): React.JSX.Element {
    leadingElement = (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="32"
            height="32"
            viewBox="0 0 32 32"
            fill="none"
        >
            <circle cx="16" cy="16" r="16" fill="#0101FF" />
            <circle
                cx="16"
                cy="16"
                r="16"
                fill="url(#paint0_linear_163_40950)"
                fill-opacity="0.2"
            />
            <circle
                cx="16"
                cy="16"
                r="16"
                fill="url(#paint1_linear_163_40950)"
                fill-opacity="0.2"
            />
            <defs>
                <linearGradient
                    id="paint0_linear_163_40950"
                    x1="16"
                    y1="0"
                    x2="23.619"
                    y2="17.9048"
                    gradientUnits="userSpaceOnUse"
                >
                    <stop offset="0.990135" stop-opacity="0" />
                    <stop offset="1" />
                </linearGradient>
                <linearGradient
                    id="paint1_linear_163_40950"
                    x1="16"
                    y1="0"
                    x2="9.14286"
                    y2="7.2381"
                    gradientUnits="userSpaceOnUse"
                >
                    <stop offset="0.9999" stop-opacity="0" />
                    <stop offset="1" />
                </linearGradient>
            </defs>
        </svg>
    );
    const renderContent = () => {
        switch (type) {
            case TableCellType.Text:
                return <span>{label}</span>;
            case TableCellType.TextToCopy:
                return (
                    <div className="flex items-center space-x-2">
                        <span>{label}</span>
                        <Copy />
                    </div>
                );
            case TableCellType.Badge:
                return <Badge type={badgeType} label={label} />;
            case TableCellType.AvatarText:
                return (
                    <div className="flex items-center space-x-2">
                        {leadingElement}
                        <span>{label}</span>
                    </div>
                );
            case TableCellType.Toggle:
                return (
                    <div className="flex items-center space-x-2">
                        <label className="inline-flex cursor-pointer items-center">
                            <input type="checkbox" value="" className="peer sr-only" />
                            <div className="peer relative h-6 w-11 rounded-full bg-gray-200 after:absolute after:start-[2px] after:top-[2px] after:h-5 after:w-5 after:rounded-full after:border after:border-gray-300 after:bg-white after:transition-all after:content-[''] peer-checked:bg-blue-600 peer-checked:after:translate-x-full peer-checked:after:border-white peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 rtl:peer-checked:after:-translate-x-full dark:border-gray-600 dark:bg-gray-700 dark:peer-focus:ring-blue-800"></div>
                            <span className="text-sm ms-3 font-medium text-gray-900 dark:text-gray-300">
                                {label}
                            </span>
                        </label>
                    </div>
                );
            default:
                return null;
        }
    };

    return <td className="p-2 border">{renderContent()}</td>;
}
