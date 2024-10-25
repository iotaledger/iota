// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { AccountSwitcher } from '@/components/molecules/account-switcher/AccountSwitcher';

const meta: Meta<typeof AccountSwitcher> = {
    component: AccountSwitcher,
    tags: ['autodocs'],
    render: (props) => {
        return (
            <div className="flex">
                <AccountSwitcher {...props} />
            </div>
        );
    },
} satisfies Meta<typeof AccountSwitcher>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        text: 'Connect',
    },
};
