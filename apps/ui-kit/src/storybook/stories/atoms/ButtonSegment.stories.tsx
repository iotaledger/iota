// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { ButtonSegment, ButtonSegmentType } from '@/components/atoms/';

const meta: Meta<typeof ButtonSegment> = {
    component: ButtonSegment,
    tags: ['autodocs'],
    render: (props) => {
        return (
            <div className="flex flex-col items-start gap-2">
                <ButtonSegment {...props} />
            </div>
        );
    },
} satisfies Meta<typeof ButtonSegment>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        label: 'Label',
    },
    argTypes: {
        label: {
            control: 'text',
        },
        selected: {
            control: 'boolean',
        },
        disabled: {
            control: 'boolean',
        },
        type: {
            control: {
                type: 'select',
                options: Object.values(ButtonSegmentType),
            },
        },
    },
};
