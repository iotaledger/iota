// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { Button } from './Button';

const meta = {
    component: Button,
    tags: ['autodocs'],
    render: (props) => {
        return (
            <div className="flex flex-col gap-2 items-start">
                <Button {...props}>Primary</Button>
            </div>
        );
    },
} satisfies Meta<typeof Button>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        label: 'Button',
    },
};
