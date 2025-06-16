// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import { Toggle, ToggleLabelPosition, ToggleSize } from '@/components';
import { useState } from 'react';

const meta: Meta<typeof Toggle> = {
    component: Toggle,
    tags: ['autodocs'],
    argTypes: {
        isToggled: {
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
        isToggled: false,
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
        const [isToggled, setIsToggled] = useState(args.isToggled);

        const handleToggleChange = (newState: boolean) => {
            console.log('Toggle state changed:', newState);
            setIsToggled(newState);
        };

        return <Toggle {...args} isToggled={isToggled} onChange={handleToggleChange} />;
    },
};
