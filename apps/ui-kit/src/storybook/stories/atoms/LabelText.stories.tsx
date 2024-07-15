// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { LabelText, LabelTextSize } from '@/components';

const meta = {
    component: LabelText,
    tags: ['autodocs'],
    render: (props) => {
        return <LabelText {...props} />;
    },
} satisfies Meta<typeof LabelText>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        value: '12,000.00',
        text: 'Label',
        size: LabelTextSize.Medium,
        showSupportingLabel: true,
        supportingLabel: 'IOTA',
        isCentered: false,
    },
    argTypes: {
        size: {
            control: 'select',
            options: Object.values(LabelTextSize),
        },
        text: {
            control: 'text',
        },
        isCentered: {
            control: 'boolean',
        },
        supportingLabel: {
            control: 'text',
        },
        showSupportingLabel: {
            control: 'boolean',
        },
        value: {
            control: 'text',
        },
    },
};
