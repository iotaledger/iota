// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ListItem } from '@/components';
import type { Meta, StoryObj } from '@storybook/react';

const meta = {
    component: ListItem,
    tags: ['autodocs'],
    render: (props) => {
        return (
            <div className="flex flex-col items-start gap-2">
                <ListItem {...props} />
            </div>
        );
    },
} satisfies Meta<typeof ListItem>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {},
    argTypes: {
        showRightIcon: {
            control: 'boolean',
        },
    },
};
