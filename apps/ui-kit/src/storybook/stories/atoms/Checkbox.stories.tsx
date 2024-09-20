// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import { Checkbox } from '@/components';

const meta: Meta<typeof Checkbox> = {
    component: Checkbox,
    tags: ['autodocs'],
    render: (props) => {
        return (
            <div className="flex">
                <Checkbox {...props} />
            </div>
        );
    },
} satisfies Meta<typeof Checkbox>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        label: 'Label',
    },
};

export const LabelFirst: Story = {
    args: {
        label: 'Label',
        isLabelFirst: true,
    },
};
