// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import {
    TableCellProps,
    TableCellType,
    TableCell,
    TableHeaderCell,
    BadgeType,
    Table,
    TableBody,
    TableHeader,
    TableRow,
} from '@/lib';

import { Globe, IotaLogoSmall } from '@iota/ui-icons';

const meta = {
    component: Table,
    tags: ['autodocs'],
    render: (props) => {
        const headers = [
            { hasCheckbox: true, columnKey: 1 },
            { label: 'Name', columnKey: 2, hasSort: true },
            { label: 'Age', columnKey: 3, hasSort: true },
            { label: 'Ocupation', columnKey: 4 },
            { label: 'Email', columnKey: 5 },
            { label: 'Start Date', columnKey: 6 },
            { label: 'End Date', columnKey: 7 },
        ];

        const rows: TableCellProps[][] = [
            [
                { type: TableCellType.Text, label: '1.' },
                {
                    type: TableCellType.AvatarText,
                    leadingElement: <IotaLogoSmall />,
                    label: 'John Doe',
                },
                { type: TableCellType.Badge, badgeType: BadgeType.PrimarySolid, label: '30' },
                { type: TableCellType.Text, label: 'Software Engineer' },
                { type: TableCellType.TextToCopy, label: 'test@acme.com' },
                { type: TableCellType.Text, label: '10.04.2016' },
                { type: TableCellType.Text, label: '12.03.2019' },
            ],
            [
                { type: TableCellType.Text, label: '2.' },
                { type: TableCellType.AvatarText, leadingElement: <Globe />, label: 'Jane Smith' },
                { type: TableCellType.Badge, badgeType: BadgeType.Neutral, label: '25' },
                { type: TableCellType.Text, label: 'Graphic Designer' },
                { type: TableCellType.TextToCopy, label: 'test@acme.com' },
                { type: TableCellType.Text, label: '10.04.2016' },
                { type: TableCellType.Text, label: '12.03.2019' },
            ],
            [
                { type: TableCellType.Text, label: '3.' },
                { type: TableCellType.AvatarText, leadingElement: <Globe />, label: 'Sam Johnson' },
                { type: TableCellType.Badge, badgeType: BadgeType.PrimarySoft, label: '40' },
                { type: TableCellType.Text, label: 'Project Manager' },
                { type: TableCellType.TextToCopy, label: 'test@acme.com' },
                { type: TableCellType.Text, label: '10.04.2016' },
                { type: TableCellType.Text, label: '12.03.2019' },
            ],
        ];

        return (
            <div className="container mx-auto p-4">
                <Table {...props}>
                    <TableHeader>
                        <TableRow>
                            {headers.map((header, index) => (
                                <TableHeaderCell key={index} {...header} />
                            ))}
                        </TableRow>
                    </TableHeader>
                    <TableBody>
                        {rows.map((row, rowIndex) => (
                            <TableRow key={rowIndex}>
                                {row.map((cell, cellIndex) => (
                                    <TableCell key={cellIndex} {...cell} />
                                ))}
                            </TableRow>
                        ))}
                    </TableBody>
                </Table>
            </div>
        );
    },
} satisfies Meta<typeof Table>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        supportingLabel: '10.7k records',
        hasPagination: true,
        actionLabel: 'Action',
    },
};
