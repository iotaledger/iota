// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { Chip } from '@/components/molecules/chip/Chip';
import { PlaceholderReplace } from '@iota/apps-ui-icons';

const meta: Meta<typeof Chip> = {
    component: Chip,
    tags: ['autodocs'],
    render: (props) => {
        return (
            <div className="flex">
                <Chip {...props} />
            </div>
        );
    },
} satisfies Meta<typeof Chip>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        label: 'Label',
    },
    argTypes: {
        label: {
            control: 'text',
        },
        showClose: {
            control: 'boolean',
        },
        selected: {
            control: 'boolean',
        },
    },
};

export const WithIcon: Story = {
    args: {
        label: 'Label',
        leadingElement: <PlaceholderReplace />,
    },
    render: (props) => {
        return (
            <div className="flex flex-row gap-x-4">
                <Chip {...props} />
                <Chip {...props} showClose />
            </div>
        );
    },
};

export const WithAvatar: Story = {
    args: {
        label: 'Label',
        avatar: <Avatar />,
    },
    render: (props) => {
        return (
            <div className="flex flex-row gap-x-4">
                <Chip {...props} />
                <Chip {...props} showClose />
            </div>
        );
    },
};

function Avatar() {
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="24"
            height="24"
            viewBox="0 0 24 24"
            fill="none"
        >
            <circle cx="12" cy="12" r="12" fill="#0101FF" />
            <circle
                cx="12"
                cy="12"
                r="12"
                fill="url(#paint0_linear_288_11913)"
                fill-opacity="0.2"
            />
            <circle
                cx="12"
                cy="12"
                r="12"
                fill="url(#paint1_linear_288_11913)"
                fill-opacity="0.2"
            />
            <defs>
                <linearGradient
                    id="paint0_linear_288_11913"
                    x1="12"
                    y1="0"
                    x2="17.7143"
                    y2="13.4286"
                    gradientUnits="userSpaceOnUse"
                >
                    <stop offset="0.990135" stop-opacity="0" />
                    <stop offset="1" />
                </linearGradient>
                <linearGradient
                    id="paint1_linear_288_11913"
                    x1="12"
                    y1="0"
                    x2="6.85714"
                    y2="5.42857"
                    gradientUnits="userSpaceOnUse"
                >
                    <stop offset="0.9999" stop-opacity="0" />
                    <stop offset="1" />
                </linearGradient>
            </defs>
        </svg>
    );
}
