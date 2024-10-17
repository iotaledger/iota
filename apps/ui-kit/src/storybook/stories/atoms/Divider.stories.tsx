// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { Divider } from '@/components/atoms/divider';
import { DividerType } from '@/components/atoms/divider';

const meta: Meta<typeof Divider> = {
    component: Divider,
    tags: ['autodocs'],
    render: (props) => {
        return (
            <div className="flex h-96 items-center justify-center">
                <Divider {...props} />
            </div>
        );
    },
} satisfies Meta<typeof Divider>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        width: 'w-3/4',
        height: 'h-3/4',
    },
    argTypes: {
        width: {
            control: 'text',
        },
        type: {
            control: {
                type: 'select',
                options: Object.values(DividerType),
            },
        },
    },
};
