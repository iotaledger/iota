// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Tooltip, TooltipPosition } from '@/lib/components';
import { Info } from '@iota/apps-ui-icons';
import type { Meta, StoryObj } from '@storybook/react';

const meta: Meta<typeof Tooltip> = {
    component: Tooltip,
    tags: ['autodocs'],
    render: (props) => {
        return (
            <div className="flex h-20 items-center p-sm">
                <Tooltip {...props}>
                    <span className="text-neutral-10 dark:text-neutral-92">Hover me</span>
                </Tooltip>
            </div>
        );
    },
} satisfies Meta<typeof Tooltip>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        text: 'This is a tooltip',
    },
    argTypes: {
        text: {
            control: {
                type: 'text',
            },
        },
        position: {
            control: {
                type: 'select',
                options: Object.values(TooltipPosition),
            },
        },
    },
};

export const WithIcon: Story = {
    args: {
        text: 'This is a tooltip',
        position: TooltipPosition.Right,
    },
    render: (props) => {
        return (
            <Tooltip {...props}>
                <Info className="text-neutral-10 dark:text-neutral-92" />
            </Tooltip>
        );
    },
};
