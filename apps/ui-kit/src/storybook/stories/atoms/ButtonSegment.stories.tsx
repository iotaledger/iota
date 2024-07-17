// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { ButtonSegment } from '@/components/atoms/';

const meta = {
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
    },
};
