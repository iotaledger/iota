// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import {
    TableHeaderCell,
    Table,
    TableBody,
    TableHeader,
    TableRow,
    TableRowCheckbox,
    TableCellText,
    TableActionButton,
} from '@/lib';
import { useState } from 'react';

const HEADERS = [
    {
        label: 'Name',
        columnKey: 2,
        hasSort: true,
        cell: ({ text }: { text: string }) => <TableCellText>{text}</TableCellText>,
    },
    {
        label: 'Age',
        columnKey: 3,
        hasSort: true,
        cell: ({ text }: { text: string }) => <TableCellText>{text}</TableCellText>,
    },
    {
        label: 'Occupation',
        columnKey: 4,
        cell: ({ text }: { text: string }) => <TableCellText>{text}</TableCellText>,
    },
    {
        label: 'Email',
        columnKey: 5,
        cell: ({ text }: { text: string }) => <TableCellText>{text}</TableCellText>,
    },
    {
        label: 'Start Date',
        columnKey: 6,
        cell: ({ text }: { text: string }) => <TableCellText>{text}</TableCellText>,
    },
    {
        label: 'End Date',
        columnKey: 7,
        cell: ({ text }: { text: string }) => <TableCellText>{text}</TableCellText>,
    },
];

const DATA = [
    {
        name: 'Jon Doe',
        age: 30,
        occupation: 'Software Engineer',
        email: 'test@acme.com',
        startDate: '10.04.2016',
        endDate: '12.03.2019',
    },
];

const meta = {
    component: Table,
    tags: ['autodocs'],
    render: (props) => {
        const [selectedRows, setSelectedRows] = useState<Set<number>>(new Set());
        return (
            <div className="container mx-auto p-4">
                <Table {...props} selectedRowIndexes={selectedRows}>
                    <TableHeader>
                        <TableRow>
                            {HEADERS.map((header, index) => (
                                <TableHeaderCell key={index} {...header} />
                            ))}
                        </TableRow>
                    </TableHeader>
                    <TableBody>
                        {DATA.map((row, rowIndex) => (
                            <TableRow
                                key={rowIndex}
                                leading={
                                    <TableRowCheckbox
                                        rowIndex={rowIndex}
                                        onCheckboxChange={(checked) => {
                                            setSelectedRows((selectedRows) => {
                                                if (checked) {
                                                    selectedRows.add(rowIndex);
                                                } else {
                                                    selectedRows.delete(rowIndex);
                                                }

                                                console.log(
                                                    'Checked checkbox at index:',
                                                    rowIndex,
                                                    'with value:',
                                                    checked,
                                                    'table values:',
                                                    selectedRows,
                                                );

                                                return new Set(selectedRows);
                                            });
                                        }}
                                    />
                                }
                            >
                                {Object.entries(row).map((cell, columnIndex) => {
                                    const Cell = HEADERS[columnIndex].cell;
                                    return (
                                        <td key={columnIndex}>
                                            <Cell text={cell[1].toString()} />
                                        </td>
                                    );
                                })}
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
        paginationOptions: {
            onFirst: () => console.log('First'),
            onNext: () => console.log('Next'),
            hasFirst: true,
            hasNext: false,
        },
        action: <TableActionButton text="Action" />,
        rowIndexes: [0, 1, 2],
    },
};
