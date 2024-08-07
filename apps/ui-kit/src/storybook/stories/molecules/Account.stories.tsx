// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import { Account, BadgeType } from '@/components';
import cx from 'classnames';

const meta: Meta<typeof Account> = {
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

const Avatar = ({ isLocked }: { isLocked?: boolean }) => {
    const circleFillClass = isLocked ? 'fill-neutral-80 dark:fill-neutral-30' : 'fill-primary-30';
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="33"
            height="32"
            viewBox="0 0 33 32"
            fill="none"
        >
            <circle cx="16.5" cy="16" r="16" className={cx(circleFillClass)} />
        </svg>
    );
};

export const Default: Story = {
    args: {
        title: 'Account',
        subtitle: '0x0d7...3f37',
        isLocked: true,
        avatarContent: Avatar,
    },
    argTypes: {
        badgeType: {
            control: 'select',
            options: Object.values(BadgeType),
        },
        isLocked: {
            control: 'boolean',
        },
        onOptionsClick: {
            action: 'onOptionsClick',
            control: 'none',
        },
        onLockAccountClick: {
            action: 'onLockAccount',
            control: 'none',
        },
        avatarContent: {
            control: 'none',
        },
        badgeText: {
            control: 'text',
        },
    },
};
