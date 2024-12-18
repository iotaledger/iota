// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Header } from '@/lib/components/atoms/header';
import type { Meta, StoryObj } from '@storybook/react';

const meta: Meta<typeof Header> = {
    component: Header,
    tags: ['autodocs'],
    render: (props) => {
        return (
            <div className="flex flex-col items-start gap-2">
                <Header {...props} />
            </div>
        );
    },
} satisfies Meta<typeof Header>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        title: 'Header',
        onBack: undefined,
        onClose: undefined,
    },
    argTypes: {
        titleCentered: {
            control: 'boolean',
        },
    },
};

export const Centered: Story = {
    args: {
        title: 'Header',
        titleCentered: true,
    },
};

export const CenteredNoIcon: Story = {
    args: {
        title: 'Header',
        titleCentered: true,
        onBack: undefined,
        onClose: undefined,
    },
};

export const NoIcon: Story = {
    args: {
        title: 'Header',
        onBack: undefined,
        onClose: undefined,
    },
};
