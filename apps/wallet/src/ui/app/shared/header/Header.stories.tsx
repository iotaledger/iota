// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type Meta, type StoryObj } from '@storybook/react';

import { Header } from './Header';

export default {
    component: Header,
} as Meta<typeof Header>;

export const Default: StoryObj<typeof Header> = {};

export const Full: StoryObj<typeof Header> = {
    args: {
        middleContent: (
            <div className="overflow-hidden text-ellipsis whitespace-nowrap">
                Connected to some dapp
            </div>
        ),
        rightContent: <div>Menu</div>,
    },
};

export const WithMiddleContentOnly: StoryObj<typeof Header> = {
    args: {
        middleContent: (
            <div className="overflow-hidden text-ellipsis whitespace-nowrap">
                Connected to some dapp
            </div>
        ),
    },
};

export const WithRightContentOnly: StoryObj<typeof Header> = {
    args: {
        rightContent: <div>Menu</div>,
    },
};
