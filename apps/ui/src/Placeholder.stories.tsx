// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type Meta, type StoryObj } from '@storybook/react';

import { Placeholder } from './Placeholder';

const meta = {
    component: Placeholder,
} satisfies Meta<typeof Placeholder>;

export default meta;

type Story = StoryObj<typeof meta>;

export const VaryingWidthAndHeight: Story = {
    render: () => (
        <div>
            <Placeholder width="120px" height="12px" />
            <br />
            <Placeholder width="90px" height="16px" />
            <br />
            <Placeholder width="59px" height="32px" />
        </div>
    ),
};
