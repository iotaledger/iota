// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { Chip } from '@/components/atoms/chip/Chip';

const meta = {
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
        icon: <Icon />,
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

function Icon() {
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="16"
            height="16"
            viewBox="0 0 16 16"
            fill="none"
        >
            <path
                d="M7.3335 6.00004C7.3335 6.73642 6.73654 7.33337 6.00016 7.33337C5.26378 7.33337 4.66683 6.73642 4.66683 6.00004C4.66683 5.26366 5.26378 4.66671 6.00016 4.66671C6.73654 4.66671 7.3335 5.26366 7.3335 6.00004Z"
                fill="currentColor"
            />
            <path
                d="M10.0002 7.33337C10.7365 7.33337 11.3335 6.73642 11.3335 6.00004C11.3335 5.26366 10.7365 4.66671 10.0002 4.66671C9.26378 4.66671 8.66683 5.26366 8.66683 6.00004C8.66683 6.73642 9.26378 7.33337 10.0002 7.33337Z"
                fill="currentColor"
            />
            <path
                fill-rule="evenodd"
                clip-rule="evenodd"
                d="M1.3335 5.33337C1.3335 3.12424 3.12436 1.33337 5.3335 1.33337H10.6668C12.876 1.33337 14.6668 3.12424 14.6668 5.33337V10.6667C14.6668 12.8758 12.876 14.6667 10.6668 14.6667H5.3335C3.12436 14.6667 1.3335 12.8758 1.3335 10.6667V5.33337ZM2.66683 5.33337C2.66683 3.86062 3.86074 2.66671 5.3335 2.66671H10.6668C12.1396 2.66671 13.3335 3.86062 13.3335 5.33337V7.91404C9.94941 9.47281 6.05092 9.47281 2.66683 7.91404V5.33337ZM2.66683 9.36758V10.6667C2.66683 12.1395 3.86074 13.3334 5.3335 13.3334H10.6668C12.1396 13.3334 13.3335 12.1395 13.3335 10.6667V9.36758C9.91659 10.7661 6.08374 10.7661 2.66683 9.36758Z"
                fill="currentColor"
            />
        </svg>
    );
}

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
