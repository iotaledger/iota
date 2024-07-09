// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { SegmentedButton } from '@/components/atoms/segmented-button/SegmentedButton';
import { SegmentedButtonType } from '@/lib/components/atoms/segmented-button';

const meta = {
    component: SegmentedButton,
    tags: ['autodocs'],
    render: (props) => {
        return (
            <div className="flex flex-col items-start gap-2">
                <SegmentedButton {...props} />
            </div>
        );
    },
} satisfies Meta<typeof SegmentedButton>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        elements: [
            { label: 'Label 1' },
            { label: 'Label 2' },
            { label: 'Label 3' },
            { label: 'Label 4' },
        ],
    },
    argTypes: {
        elements: {
            control: 'object',
        },
        type: {
            control: {
                type: 'select',
                options: Object.values(SegmentedButtonType),
            },
        },
    },
};
