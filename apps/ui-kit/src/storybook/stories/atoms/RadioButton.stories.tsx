// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { RadioButton } from '@/components/atoms';

const meta = {
    component: RadioButton,
    tags: ['autodocs'],
    render: (props) => {
        return <RadioButton {...props} />;
    },
} satisfies Meta<typeof RadioButton>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        checked: true,
    },
    argTypes: {
        checked: {
            control: 'boolean',
        },
    },
};
