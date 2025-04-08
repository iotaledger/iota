// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import { Toggle, ToggleLabelPosition, ToggleSize } from '@/components';
import { useState } from 'react';

const meta: Meta<typeof Toggle> = {
    component: Toggle,
    tags: ['autodocs'],
    argTypes: {
        isActive: {
            control: { type: 'boolean' },
        },
        label: {
            control: { type: 'text' },
        },
        labelPosition: {
            control: { type: 'select' },
            options: Object.values(ToggleLabelPosition),
        },
        isDisabled: {
            control: { type: 'boolean' },
        },
        size: {
            control: { type: 'select' },
            options: Object.values(ToggleSize),
        },
        onChange: {
            action: 'changed',
        },
    },
    args: {
        isActive: false,
        isDisabled: false,
        size: ToggleSize.Default,
        labelPosition: ToggleLabelPosition.Right,
    },
} satisfies Meta<typeof Toggle>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        label: 'Label',
    },
    render: (args) => {
        const [isActive, setIsActive] = useState(args.isActive);

        const handleToggleChange = (newActiveState: boolean) => {
            console.log('Toggle state changed:', newActiveState);
            setIsActive(newActiveState);
        };

        return <Toggle {...args} isActive={isActive} onChange={handleToggleChange} />;
    },
};
