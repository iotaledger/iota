// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import { BadgeType, TableCell, TableCellType } from '@/components';

const meta = {
    component: TableCell,
    tags: ['autodocs'],
    render: (props) => {
        return (
            <table>
                <thead>
                    <tr>
                        <TableCell {...props} />
                    </tr>
                </thead>
            </table>
        );
    },
} satisfies Meta<typeof TableCell>;

export default meta;

type Story = StoryObj<typeof meta>;

const Avatar = () => {
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="33"
            height="32"
            viewBox="0 0 33 32"
            fill="none"
        >
            <circle cx="16.5" cy="16" r="16" className="fill-primary-40" />
        </svg>
    );
};

export const Default: Story = {
    args: {
        type: TableCellType.AvatarText,
        label: 'Label',
        leadingElement: <Avatar />,
    },
    argTypes: {
        badgeType: {
            control: 'select',
            options: Object.values(BadgeType),
        },
        type: {
            control: 'select',
            options: Object.values(TableCellType),
        },
        leadingElement: {
            control: 'none',
        },
        label: {
            control: 'text',
        },
        supportingLabel: {
            control: 'text',
        },
    },
};
