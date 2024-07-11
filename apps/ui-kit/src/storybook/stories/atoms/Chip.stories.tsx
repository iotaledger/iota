// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { Chip } from '@/components/atoms/chip/Chip';

const meta = {
    component: Chip,
    tags: ['autodocs'],
    render: (props) => {
        return (
            <div className="flex">
                <Chip {...props} />
            </div>
        );
    },
} satisfies Meta<typeof Chip>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        label: 'Label',
    },
    argTypes: {
        label: {
            control: 'text',
        },
        showClose: {
            control: 'boolean',
        },
        selected: {
            control: 'boolean',
        },
        leadingElement: {
            control: 'none',
        },
    },
};
