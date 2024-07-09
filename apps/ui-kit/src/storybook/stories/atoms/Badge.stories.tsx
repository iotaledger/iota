// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import { Badge, BadgeVariant } from '@/components';

const meta = {
    component: Badge,
    tags: ['autodocs'],
    render: (props) => {
        return (
            <div className="flex flex-col items-start">
                <Badge {...props} />
            </div>
        );
    },
} satisfies Meta<typeof Badge>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        label: 'Badge',
    },
    argTypes: {
        variant: {
            control: {
                type: 'select',
                options: Object.values(BadgeVariant),
            },
        },
        disabled: {
            control: 'boolean',
        },
        icon: {
            control: 'text',
        },
    },
};
