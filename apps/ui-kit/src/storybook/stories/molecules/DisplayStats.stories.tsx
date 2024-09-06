// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import type { Meta, StoryObj } from '@storybook/react';
import {
    DisplayStats,
    TooltipPosition,
    DisplayStatsBackground,
    DisplayStatsSize,
} from '@/components';

const meta = {
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
        backgroundColor: {
            control: {
                type: 'select',
                options: Object.values(DisplayStatsBackground),
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
