// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { SegmentButton } from '@/components/atoms/';

const meta = {
    component: SegmentButton,
    tags: ['autodocs'],
    render: (props) => {
        return (
            <div className="flex flex-col items-start gap-2">
                <SegmentButton {...props} />
            </div>
        );
    },
} satisfies Meta<typeof SegmentButton>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        text: 'Label',
    },
    argTypes: {
        text: {
            control: 'text',
        },
        selected: {
            control: 'boolean',
        },
        disabled: {
            control: 'boolean',
        },
    },
};
