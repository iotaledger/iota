// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { StoryObj } from '@storybook/react';

const meta = {
    title: 'Token Showcase/Scrollbar',
    tags: ['autodocs'],
    render: () => {
        return (
            <div className="h-[300px] w-1/2 overflow-y-scroll">
                <div className="h-[500px]"></div>
            </div>
        );
    },
};

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {};
