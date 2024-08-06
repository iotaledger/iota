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
import { useState } from 'react';

const meta = {
    component: Table,
    tags: ['autodocs'],
    render: (props) => {
        const [headerChecked, setHeaderChecked] = useState(false);
        const [headerIndeterminate, setHeaderIndeterminate] = useState(false);
        const [rows, setRows] = useState([
            { id: 1, checked: false },
            { id: 2, checked: false },
            { id: 3, checked: false },
        ]);

        const handleHeaderCheckboxChange = (checked: boolean) => {
            setHeaderChecked(checked);
            setRows(rows.map((row) => ({ ...row, checked })));
            setHeaderIndeterminate(false);
        };

        const handleRowCheckboxChange = (rowIndex: number, checked: boolean) => {
            const updatedRows = rows.map((row, index) =>
                index === rowIndex ? { ...row, checked } : row,
            );
            setRows(updatedRows);

            const allChecked = updatedRows.every((row) => row.checked);
            const anyChecked = updatedRows.some((row) => row.checked);

            setHeaderChecked(allChecked);
            setHeaderIndeterminate(!allChecked && anyChecked);
        };

        const headersData = [
            {
                hasCheckbox: true,
                columnKey: 1,
                isChecked: headerChecked,
                isIndeterminate: headerIndeterminate,
                onCheckboxChange: handleHeaderCheckboxChange,
            },
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
                    type: TableCellType.Checkbox,
                    isChecked: rows[0].checked,
                    onChange: (checked) => handleRowCheckboxChange(0, checked),
                },
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
                {
                    type: TableCellType.Checkbox,
                    isChecked: rows[1].checked,
                    onChange: (checked) => handleRowCheckboxChange(1, checked),
                },
                { type: TableCellType.AvatarText, leadingElement: <Globe />, label: 'Jane Smith' },
                { type: TableCellType.Badge, badgeType: BadgeType.Neutral, label: '25' },
                { type: TableCellType.Text, label: 'Graphic Designer' },
                { type: TableCellType.TextToCopy, label: 'test@acme.com' },
                { type: TableCellType.Text, label: '10.04.2016' },
                { type: TableCellType.Text, label: '12.03.2019' },
            ],
            [
                {
                    type: TableCellType.Checkbox,
                    isChecked: rows[2].checked,
                    onChange: (checked) => handleRowCheckboxChange(2, checked),
                },
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
                            {headersData.map((header, index) => (
                                <TableHeaderCell key={index} {...header} />
                            ))}
                        </TableRow>
                    </TableHeader>
                    <TableBody>
                        {rowsData.map((row, rowIndex) => (
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
