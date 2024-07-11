// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Divider, type DividerProps } from '~/components/ui';

import type { Meta, StoryObj } from '@storybook/react';

export default {
    component: Divider,
} as Meta;

export const Horizontal: StoryObj<DividerProps> = {
    render: () => <Divider />,
};

export const Vertical: StoryObj<DividerProps> = {
    render: () => (
        <div className="flex h-[100px] gap-2">
            <div className="h-[100px] w-[100px] bg-iota" />
            <Divider vertical />
            <div className="h-[100px] w-[100px] bg-iota" />
        </div>
    ),
};
