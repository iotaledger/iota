// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import { Placeholder } from '@/components';

const meta: Meta<typeof Placeholder> = {
    component: Placeholder,
    tags: ['autodocs'],
    render: (props) => {
        return <Placeholder {...props} />;
    },
} satisfies Meta<typeof Placeholder>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {},
    argTypes: {
        width: {
            control: {
                type: 'text',
            },
        },
        height: {
            control: {
                type: 'text',
            },
        },
    },
};
