// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import type { Meta, StoryObj } from '@storybook/react';
import { Header } from '@/components';

const meta = {
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
    },
    argTypes: {
        hasLeftIcon: {
            control: 'boolean',
        },
        hasRightIcon: {
            control: 'boolean',
        },
    },
};
