// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TableHeader } from '@/lib/components/atoms';
import type { Meta, StoryObj } from '@storybook/react';

const meta: Meta<typeof TableHeader> = {
    component: TableHeader,
    tags: ['autodocs'],
    render: (props) => {
        return (
            <div className="flex w-48">
                <TableHeader {...props} />
            </div>
        );
    },
} satisfies Meta<typeof TableHeader>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        text: 'Header',
    },
    argTypes: {
        text: {
            control: 'text',
        },
    },
};
