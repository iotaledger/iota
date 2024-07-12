// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import { Account, AccountType } from '@/components';

const meta = {
    component: Account,
    tags: ['autodocs'],
    render: (props) => {
        return (
            <div className="w-1/2">
                <Account {...props} />
            </div>
        );
    },
} satisfies Meta<typeof Account>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        title: 'Account',
        subtitle: '0x0d7...3f37',
    },
    argTypes: {
        accountType: {
            control: 'select',
            options: Object.values(AccountType),
        },
        isLocked: {
            control: 'boolean',
        },
        onThreeDotsClick: {
            action: 'onThreeDotsClick',
            control: 'none',
        },
        onLockAccount: {
            action: 'onLockAccount',
            control: 'none',
        },
    },
};
