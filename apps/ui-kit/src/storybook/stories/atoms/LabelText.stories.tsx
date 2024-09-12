// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { LabelText, LabelTextSize, TooltipPosition } from '@/components';

const meta: Meta<typeof LabelText> = {
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
        text: '12,000.00',
        label: 'Label',
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
        label: {
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
        text: {
            control: 'text',
        },
        tooltipText: {
            control: 'text',
        },
        tooltipPosition: {
            control: {
                type: 'select',
                options: Object.values(TooltipPosition),
            },
        },
    },
};
