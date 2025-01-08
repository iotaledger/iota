// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import { DisplayStats, TooltipPosition, DisplayStatsType, DisplayStatsSize } from '@/components';

const meta: Meta<typeof DisplayStats> = {
    component: DisplayStats,
    tags: ['autodocs'],
    render: (props) => {
        return (
            <div className="w-1/3">
                <DisplayStats {...props} />
            </div>
        );
    },
} satisfies Meta<typeof DisplayStats>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        label: 'Label',
        value: 'Value',
        supportingLabel: 'IOTA',
        tooltipText: 'Tooltip',
        type: DisplayStatsType.Default,
    },
    argTypes: {
        label: {
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
        value: {
            control: 'text',
        },
        supportingLabel: {
            control: 'text',
        },
        type: {
            control: {
                type: 'select',
                options: Object.values(DisplayStatsType),
            },
        },
        size: {
            control: {
                type: 'select',
                options: Object.values(DisplayStatsSize),
            },
        },
    },
};
