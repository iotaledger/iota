// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { Address } from '@/components';

const meta: Meta<typeof Address> = {
    component: Address,
    tags: ['autodocs'],
    render: (props) => {
        return (
            <div className="flex flex-col items-start gap-2">
                <Address {...props} />
            </div>
        );
    },
} satisfies Meta<typeof Address>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        text: '0x0d7...3f37',
    },
    argTypes: {
        text: {
            control: 'text',
        },
        isCopyable: {
            control: 'boolean',
        },
        isExternal: {
            control: 'boolean',
        },
    },
};
