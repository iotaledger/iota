// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import { KeyValueInfo, TooltipPosition, ValueSize } from '@/components';

const meta: Meta<typeof KeyValueInfo> = {
    component: KeyValueInfo,
    tags: ['autodocs'],
    render: (props) => {
        return (
            <div className="w-1/2">
                <KeyValueInfo {...props} />
            </div>
        );
    },
} satisfies Meta<typeof KeyValueInfo>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        keyText: 'Label',
        valueText: 'Value',
        supportingLabel: 'IOTA',
        size: ValueSize.Small,
    },
    argTypes: {
        keyText: {
            control: 'text',
        },
        valueText: {
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
        supportingLabel: {
            control: 'text',
        },
        valueLink: {
            control: {
                type: 'text',
            },
        },
        size: {
            control: {
                type: 'select',
                options: Object.values(ValueSize),
            },
        },
    },
};
