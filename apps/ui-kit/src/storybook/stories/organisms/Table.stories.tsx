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
    TableHeaderRow,
    TableBodyRow,
} from '@/lib';
import { Globe, IotaLogoSmall } from '@iota/ui-icons';

const headersData = [
    { label: 'Name', columnKey: 2, hasSort: true },
    { label: 'Age', columnKey: 3, hasSort: true },
    { label: 'Occupation', columnKey: 4 },
    { label: 'Email', columnKey: 5 },
    { label: 'Start Date', columnKey: 6 },
    { label: 'End Date', columnKey: 7 },
];

const rowsData: TableCellProps[][] = [
    [
        {
            type: TableCellType.AvatarText,
            leadingElement: <IotaLogoSmall />,
            label: 'John Doe',
        },
        { type: TableCellType.Badge, badgeType: BadgeType.PrimarySolid, label: '30' },
        { type: TableCellType.Text, label: 'Software Engineer' },
        { type: TableCellType.TextToCopy, label: 'test@acme.com', textToCopy: 'test@acme.com' },
        { type: TableCellType.Text, label: '10.04.2016' },
        { type: TableCellType.Text, label: '12.03.2019' },
    ],
    [
        { type: TableCellType.AvatarText, leadingElement: <Globe />, label: 'Jane Smith' },
        { type: TableCellType.Badge, badgeType: BadgeType.Neutral, label: '25' },
        { type: TableCellType.Text, label: 'Graphic Designer' },
        { type: TableCellType.TextToCopy, label: 'test@acme.com', textToCopy: 'test@acme.com' },
        { type: TableCellType.Text, label: '10.04.2016' },
        { type: TableCellType.Text, label: '12.03.2019' },
    ],
    [
        { type: TableCellType.Placeholder },
        { type: TableCellType.Placeholder },
        { type: TableCellType.Placeholder },
        { type: TableCellType.Placeholder },
        { type: TableCellType.Placeholder },
        { type: TableCellType.Placeholder },
    ],
    [
        { type: TableCellType.AvatarText, leadingElement: <Globe />, label: 'Sam Johnson' },
        { type: TableCellType.Badge, badgeType: BadgeType.PrimarySoft, label: '40' },
        { type: TableCellType.Text, label: 'Project Manager' },
        { type: TableCellType.TextToCopy, label: 'test@acme.com', textToCopy: 'test@acme.com' },
        { type: TableCellType.Text, label: '10.04.2016' },
        { type: TableCellType.Text, label: '12.03.2019' },
    ],
];
const meta = {
    component: Table,
    tags: ['autodocs'],
    args: {
        hasCheckboxColumn: true,
        onRowCheckboxChange: (value, index, values) =>
            console.log(
                'Checked checkbox at index:',
                index,
                'with value:',
                value,
                'table values:',
                values,
            ),
        onHeaderCheckboxChange: (value) =>
            console.log('Checked header checkbox with value:', value),
    },
    render: (props) => {
        return (
            <div className="container mx-auto p-4">
                <Table {...props}>
                    <TableHeader>
                        <TableHeaderRow>
                            {headersData.map((header, index) => (
                                <TableHeaderCell key={index} {...header} />
                            ))}
                        </TableHeaderRow>
                    </TableHeader>
                    <TableBody>
                        {rowsData.map((row, rowIndex) => (
                            <TableBodyRow key={rowIndex} rowIndex={rowIndex}>
                                {row.map((cell, cellIndex) => (
                                    <TableCell key={cellIndex} {...cell} />
                                ))}
                            </TableBodyRow>
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
        paginationOptions: {
            onFirstPageClick: () => console.log('First'),
            onNextPageClick: () => console.log('Next'),
        },
        actionLabel: 'Action',
        hasCheckboxColumn: true,
        rowIndexes: [0, 1, 2],
    },
};
