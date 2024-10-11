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
                <Button {...props}>Medium</Button>
                <Button {...props} asChild>
                    <a href="https://google.com">asChild (link)</a>
                </Button>
                <Button size="lg" {...props}>
                    Large
                </Button>
                <Button disabled {...props}>
                    Disabled
                </Button>
                <Button loading {...props}>
                    Loading
                </Button>
            </div>
        );
    },
} satisfies Meta<typeof Button>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Primary: Story = {
    args: {
        variant: 'primary',
    },
};

export const Secondary: Story = {
    args: {
        variant: 'secondary',
    },
};

export const Outline: Story = {
    args: {
        variant: 'outline',
    },
};
