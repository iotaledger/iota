// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import { TableHeaderCell } from '@/components';

const meta = {
    component: TableHeaderCell,
    tags: ['autodocs'],
    render: (props) => {
        return (
            <div className="w-56">
                <table>
                    <thead>
                        <tr>
                            <TableHeaderCell {...props} />
                        </tr>
                    </thead>
                </table>
            </div>
        );
    },
} satisfies Meta<typeof TableHeaderCell>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        label: 'Label',
        columnKey: '1',
    },
};
