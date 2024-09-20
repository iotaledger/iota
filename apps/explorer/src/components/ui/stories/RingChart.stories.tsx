// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type Meta, type StoryObj } from '@storybook/react';

import { RingChart, type RingChartProps } from '~/components/ui';

export default {
    component: RingChart,
} as Meta;

export const Default: StoryObj<RingChartProps> = {
    args: {
        data: [
            { value: 63, label: 'Active', color: '#589AEA' },
            { value: 2, label: 'New', color: '#6FBCF0' },
            { value: 4, label: 'At Risk', color: '#FF794B' },
        ],
    },
};
