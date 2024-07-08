// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { Divider } from '@/components/atoms/divider';
import { DividerType } from '@/components/atoms/divider';

const meta = {
    component: Divider,
    tags: ['autodocs'],
    render: (props, context) => {
        // const height = props.type === DividerType.Horizontal ? 'h-1' : 'w-1';
        const baseStyle = {
            height: '500px',
        };
        return (
            <div className="flex flex-col items-start gap-2" style={baseStyle}>
                <Divider {...props} />
            </div>
        );
    },
} satisfies Meta<typeof Divider>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    argTypes: {
        width: {
            control: 'text',
        },
        color: {
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
