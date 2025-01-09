// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { LoadingIndicator } from '@/lib/components/atoms';
import type { Meta, StoryObj } from '@storybook/react';

const meta: Meta<typeof LoadingIndicator> = {
    component: LoadingIndicator,
    tags: ['autodocs'],
    render: (props) => {
        return <LoadingIndicator {...props} />;
    },
} satisfies Meta<typeof LoadingIndicator>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {},
    argTypes: {
        color: {
            control: 'text',
        },
        size: {
            control: 'text',
        },
        text: {
            control: 'text',
        },
    },
};
