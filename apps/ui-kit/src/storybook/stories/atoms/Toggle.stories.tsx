// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import { Toggle } from '@/components';

const meta: Meta<typeof Toggle> = {
    component: Toggle,
    tags: ['autodocs'],
    argTypes: {
        isActive: {
            control: { type: 'boolean' },
            description: 'The state of the toggle (on or off)',
        },
        label: {
            control: { type: 'text' },
            description: 'The label for the toggle',
        },
        labelPosition: {
            control: { type: 'select' },
            options: ['left', 'right'],
            description: 'Position of the label relative to the toggle',
        },
        disabled: {
            control: { type: 'boolean' },
            description: 'If true, the toggle will be disabled',
        },
        size: {
            control: { type: 'select' },
            options: ['sm', 'md'],
            description: 'Size of the toggle',
        },
        onChange: {
            action: 'changed',
            description: 'Callback when toggle state changes',
        },
    },
    args: {
        isActive: false,
        disabled: false,
        size: 'md',
        labelPosition: 'right',
    },
} satisfies Meta<typeof Toggle>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        label: 'Toggle Label',
    },
};

export const Small: Story = {
    args: {
        label: 'Small Toggle',
        size: 'sm',
    },
};

export const Disabled: Story = {
    args: {
        label: 'Disabled Toggle',
        disabled: true,
    },
};

export const LabelOnLeft: Story = {
    args: {
        label: 'Label on left',
        labelPosition: 'left',
    },
};

export const WithoutLabel: Story = {
    args: {
        label: undefined,
    },
};
