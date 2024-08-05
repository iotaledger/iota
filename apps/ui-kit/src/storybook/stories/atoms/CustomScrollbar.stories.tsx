// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CustomScrollbar } from '@/components/atoms/custom-scrollbar/CustomScrollbar';
import type { Meta, StoryObj } from '@storybook/react';

const meta = {
    component: CustomScrollbar,
    tags: ['autodocs'],
    render: () => {
        return (
            <div>
                <CustomScrollbar />
            </div>
        );
    },
} satisfies Meta<typeof CustomScrollbar>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {},
    argTypes: {},
};
