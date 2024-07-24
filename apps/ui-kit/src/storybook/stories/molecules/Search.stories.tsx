// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import type { Meta, StoryObj } from '@storybook/react';
import { Search } from '@/components';

const meta: Meta<typeof Search> = {
    component: Search,
    tags: ['autodocs'],
    render: (props) => (
        <div className="h-60">
            <Search {...props} />
        </div>
    ),
};

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        suggestions: ['Wallet', 'Explorer', 'Dashboard', 'EVM Toolkit'],
        placeholder: 'Search for tooling apps',
    },
    argTypes: {
        suggestions: {
            control: 'array',
        },
        onSuggestionClick: {
            action: 'suggestionClicked',
        },
        placeholder: {
            control: 'text',
        },
        onChange: {
            action: 'inputChanged',
        },
        type: {
            control: {
                type: 'select',
                options: ['outlined', 'filled'],
            },
        },
    },
};
