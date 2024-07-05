// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { Address } from '@/components/atoms/address';
import { AddressType } from '@/components/atoms/address/address.enums';

const meta = {
    component: Address,
    tags: ['autodocs'],
    render: (props, context) => {
        const { darkmode } = context;
        return (
            <div className="flex flex-col items-start gap-2">
                <Address {...props} darkmode={darkmode} />
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
        type: {
            control: {
                type: 'select',
                options: Object.values(AddressType),
            },
        },
        showCopy: {
            control: 'boolean',
        },
        showOpen: {
            control: 'boolean',
        },
    },
};
