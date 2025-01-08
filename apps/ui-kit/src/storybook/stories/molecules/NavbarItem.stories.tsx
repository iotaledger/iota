// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import { NavbarItem, NavbarItemType } from '@/components';
import { Home } from '@iota/ui-icons';

const meta: Meta<typeof NavbarItem> = {
    component: NavbarItem,
    tags: ['autodocs'],
    render: (props) => {
        return <NavbarItem {...props} />;
    },
} satisfies Meta<typeof NavbarItem>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        icon: <Home />,
    },
    argTypes: {
        text: {
            control: 'text',
        },
        icon: {
            control: 'none',
        },
        hasBadge: {
            control: 'boolean',
        },
        isSelected: {
            control: 'boolean',
        },
        type: {
            control: 'select',
            options: Object.values(NavbarItemType),
        },
        isDisabled: {
            control: 'boolean',
        },
    },
};
