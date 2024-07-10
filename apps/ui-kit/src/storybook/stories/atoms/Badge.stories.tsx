// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import { Badge, BadgeType } from '@/components';

const meta = {
    component: Badge,
    tags: ['autodocs'],
    render: (props) => {
        return <Badge {...props} />;
    },
} satisfies Meta<typeof Badge>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        label: 'Badge',
    },
    argTypes: {
        type: {
            control: {
                type: 'select',
                options: Object.values(BadgeType),
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
